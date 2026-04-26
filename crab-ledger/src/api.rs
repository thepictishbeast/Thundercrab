//! HTTP API surface.
//!
//! Three endpoints, no authentication, all JSON.
//!
//! - `POST /v1/submit` — accepts a single `SignedSuggestion`, verifies
//!   the signature and pattern_hash, upserts.
//! - `GET /v1/suggestions?min_corroborators=N&since_day=D` — aggregated
//!   list of patterns with their corroboration counts.
//! - `GET /v1/health` — totals; useful for monitoring.
//!
//! SECURITY:
//!   * Body size capped at 16 KiB via `tower_http::limit::RequestBodyLimitLayer`.
//!   * No CORS — clients are not browsers.
//!   * `X-Robots-Tag: noindex, nofollow` on every response so search
//!     engines (if anyone exposes the clearnet side) don't crawl.
//!   * Errors return generic strings — no leaking server internals.

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use http::header::{HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thundercrab_suggestions::{SignedSuggestion, ledger::verify};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tracing::warn;

use crate::store::{CorroborationRow, Store};

/// Hard upper bound on the size of a single submission body. Real
/// payloads are well under 1 KiB; this is defense against amplification.
const MAX_BODY_BYTES: usize = 16 * 1024;

/// Shared service state. Wrapped in `Arc` for the Axum extractor.
pub struct AppState {
    /// SQLite-backed corroboration store. Wrapped in a `Mutex` to
    /// serialize writes — SQLite's WAL mode allows concurrent readers
    /// but a single writer per connection.
    pub store: tokio::sync::Mutex<Store>,
}

/// Build the router. Caller wires it up in `main`.
#[must_use]
pub fn router(state: Arc<AppState>) -> Router {
    let robots = SetResponseHeaderLayer::overriding(
        HeaderName::from_static("x-robots-tag"),
        HeaderValue::from_static("noindex, nofollow"),
    );
    let no_referrer = SetResponseHeaderLayer::overriding(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );

    Router::new()
        .route("/v1/submit", post(handle_submit))
        .route("/v1/suggestions", get(handle_suggestions))
        .route("/v1/health", get(handle_health))
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(robots)
        .layer(no_referrer)
        .with_state(state)
}

#[derive(Serialize)]
struct SubmitResponse {
    /// `true` if this is a new corroborator for the pattern.
    new_corroborator: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

async fn handle_submit(
    State(state): State<Arc<AppState>>,
    Json(signed): Json<SignedSuggestion>,
) -> impl IntoResponse {
    let suggestion = match verify(&signed) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "rejected submission: signature/hash failure");
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "verification_failed",
                }),
            )
                .into_response();
        }
    };

    let rule_json = match canonical_rule_json(&suggestion.rule) {
        Ok(j) => j,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "encode_failure",
                }),
            )
                .into_response();
        }
    };

    let row = CorroborationRow {
        pattern_hash: suggestion.pattern_hash,
        public_key: signed.public_key,
        signature: signed.signature,
        rule_json,
        submitted_day: suggestion.submitted_at_day,
    };

    let guard = state.store.lock().await;
    match guard.upsert(&row) {
        Ok(new_corroborator) => (
            StatusCode::ACCEPTED,
            Json(SubmitResponse { new_corroborator }),
        )
            .into_response(),
        Err(e) => {
            warn!(error = %e, "store upsert failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "store_failure",
                }),
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
struct SuggestionsQuery {
    /// Minimum corroborator count to include. Defaults to 1.
    /// Clients should request 3+ for production trust.
    #[serde(default = "default_min_corroborators")]
    min_corroborators: i64,
    /// Only include patterns first seen on or after this day index.
    /// Defaults to no filter.
    #[serde(default)]
    since_day: Option<i64>,
    /// Cap returned rows. Defaults to 1000.
    #[serde(default = "default_limit")]
    limit: usize,
}

const fn default_min_corroborators() -> i64 {
    1
}

const fn default_limit() -> usize {
    1000
}

#[derive(Serialize)]
struct SuggestionEntry {
    pattern_hash: String,
    rule: serde_json::Value,
    corroborators: i64,
    first_seen_day: i64,
}

#[derive(Serialize)]
struct SuggestionsResponse {
    suggestions: Vec<SuggestionEntry>,
}

async fn handle_suggestions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SuggestionsQuery>,
) -> impl IntoResponse {
    let guard = state.store.lock().await;
    let limit = q.limit.min(default_limit());
    let aggregated = match guard.list_aggregated(q.min_corroborators, q.since_day) {
        Ok(rows) => rows,
        Err(e) => {
            warn!(error = %e, "list_aggregated failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "store_failure",
                }),
            )
                .into_response();
        }
    };

    let suggestions: Vec<SuggestionEntry> = aggregated
        .into_iter()
        .take(limit)
        .map(|a| SuggestionEntry {
            pattern_hash: a.pattern_hash_hex,
            rule: serde_json::from_str(&a.rule_json)
                .unwrap_or(serde_json::Value::Null),
            corroborators: a.corroborators,
            first_seen_day: a.first_seen_day,
        })
        .collect();

    (StatusCode::OK, Json(SuggestionsResponse { suggestions })).into_response()
}

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    total_unique_patterns: i64,
}

async fn handle_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let guard = state.store.lock().await;
    let total = guard.total_unique_patterns().unwrap_or(0);
    Json(HealthResponse {
        ok: true,
        total_unique_patterns: total,
    })
}

/// Canonicalize a rule (sorted keys) before storing. Same canonical
/// encoding the suggestions crate uses for signing — we want the
/// stored body to be byte-identical to what was signed.
fn canonical_rule_json(rule: &thundercrab_core::CrabRule) -> Result<String, serde_json::Error> {
    let v: serde_json::Value = serde_json::to_value(rule)?;
    let canonical = sort_value(v);
    serde_json::to_string(&canonical)
}

fn sort_value(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = serde_json::Map::new();
            for (k, child) in entries {
                out.insert(k, sort_value(child));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(sort_value).collect())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use http::Request;
    use thundercrab_core::{Action, CrabRule, MatchExpr, RuleOrigin};
    use thundercrab_suggestions::ledger::{InstallKey, Suggestion};
    use tower::ServiceExt;

    fn fixture_rule(folder: &str) -> CrabRule {
        CrabRule {
            id: format!("test_{folder}").to_lowercase(),
            display_name: format!("Test {folder}").into(),
            when: MatchExpr::HasHeader {
                header: "List-Unsubscribe".into(),
            },
            action: Action::FileInto {
                folder: folder.into(),
            },
            score: 30,
            stop_on_match: true,
            origin: RuleOrigin::User,
        }
    }

    fn build_app() -> Router {
        let store = Store::open_in_memory().unwrap();
        let state = Arc::new(AppState {
            store: tokio::sync::Mutex::new(store),
        });
        router(state)
    }

    #[tokio::test]
    async fn health_returns_zero_on_fresh_store() {
        let app = build_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 4 * 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["ok"], true);
        assert_eq!(json["total_unique_patterns"], 0);
    }

    #[tokio::test]
    async fn submit_then_suggestions_round_trip() {
        let app = build_app();
        let key = InstallKey::generate();
        let signed = key.sign(Suggestion::new(fixture_rule("Promotions")));
        let body = serde_json::to_vec(&signed).unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/submit")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let resp_body = to_bytes(resp.into_body(), 4 * 1024).await.unwrap();
        let resp_json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert_eq!(resp_json["new_corroborator"], true);

        // GET /v1/suggestions sees it.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/suggestions?min_corroborators=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 16 * 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = json["suggestions"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["corroborators"], 1);
        assert_eq!(arr[0]["rule"]["id"], "test_promotions");
    }

    #[tokio::test]
    async fn duplicate_submission_does_not_double_count() {
        let app = build_app();
        let key = InstallKey::generate();
        let signed = key.sign(Suggestion::new(fixture_rule("Updates")));
        let body = serde_json::to_vec(&signed).unwrap();

        for _ in 0..3 {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/submit")
                        .header("content-type", "application/json")
                        .body(Body::from(body.clone()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::ACCEPTED);
        }

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/suggestions?min_corroborators=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(resp.into_body(), 16 * 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = json["suggestions"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["corroborators"], 1);
    }

    #[tokio::test]
    async fn three_distinct_keys_count_as_three() {
        let app = build_app();
        let rule = fixture_rule("Promotions");
        for _ in 0..3 {
            let k = InstallKey::generate();
            let s = k.sign(Suggestion::new(rule.clone()));
            let body = serde_json::to_vec(&s).unwrap();
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/submit")
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::ACCEPTED);
        }
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/suggestions?min_corroborators=3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(resp.into_body(), 16 * 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["suggestions"].as_array().unwrap()[0]["corroborators"], 3);
    }

    #[tokio::test]
    async fn forged_signature_is_rejected() {
        let app = build_app();
        let key = InstallKey::generate();
        let mut signed = key.sign(Suggestion::new(fixture_rule("Promotions")));
        // Flip a bit in the signature — must fail verify().
        signed.signature[0] ^= 0xff;
        let body = serde_json::to_vec(&signed).unwrap();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/submit")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn body_size_limit_rejects_oversized_payload() {
        let app = build_app();
        // 32 KiB of JSON garbage. Should hit the 16 KiB cap before
        // body parsing even attempts.
        let big = vec![b'A'; 32 * 1024];
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/submit")
                    .header("content-type", "application/json")
                    .body(Body::from(big))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Axum 0.8 returns 413 for over-limit payloads.
        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn malformed_json_returns_400() {
        let app = build_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/submit")
                    .header("content-type", "application/json")
                    .body(Body::from("{not json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(matches!(
            resp.status(),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        ));
    }

    #[tokio::test]
    async fn responses_carry_robots_noindex() {
        let app = build_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let xrt = resp.headers().get("x-robots-tag").unwrap();
        assert_eq!(xrt.to_str().unwrap(), "noindex, nofollow");
    }
}
