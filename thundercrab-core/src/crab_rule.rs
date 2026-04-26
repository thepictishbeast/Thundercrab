//! `CrabRule` — the wire schema for a sorting rule.
//!
//! **Wire-compatible** with `mail_config::CategoryRule` from the
//! Secure-Email-Server-and-UI repo. Identical serde tags, identical AST.
//! When the orchestrator emits a Sieve script from `CategoryRule` and
//! Thundercrab edits the same rule via ManageSieve, the JSON
//! representation must be byte-stable across the two crates.
//!
//! BUG ASSUMPTION: If `mail_config::CategoryRule` adds a variant,
//! `CrabRule` must add it on the same release. The integration test in
//! the `thundercrab-suggestions` crate (`schema_compat`) catches drift
//! by deserializing fixtures.

use serde::{Deserialize, Serialize};

/// AST of a header-only match expression. Mirrors
/// `mail_config::MatchExpr` byte-for-byte.
///
/// SECURITY: Only references headers, Subject, and From domain — no
/// body, no full address. A reviewer noticing a body-touching variant
/// here should reject the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(missing_docs)] // payload fields described in variant-level docs
pub enum MatchExpr {
    /// Always true. Default-route trailer.
    Always,
    /// Header `name` contains `substring` (case-insensitive).
    HeaderContains { header: String, substring: String },
    /// Header `name` is present (any value).
    HasHeader { header: String },
    /// `From:` address ends with one of the listed domains
    /// (e.g., `["@github.com"]`). Leading `@` required.
    FromDomainIn { domains: Vec<String> },
    /// `Subject:` contains any listed substring (case-insensitive).
    SubjectContainsAny { needles: Vec<String> },
    /// All sub-expressions match.
    All { exprs: Vec<MatchExpr> },
    /// Any sub-expression matches.
    Any { exprs: Vec<MatchExpr> },
    /// Sub-expression does not match.
    Not { expr: Box<MatchExpr> },
}

/// Action when a rule fires. Mirrors `mail_config::Action`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(missing_docs)] // payload fields described in variant-level docs
pub enum Action {
    /// Move to the named folder; Sieve `:create`s if missing.
    FileInto { folder: String },
    /// Set an IMAP flag (`\Flagged`, `$Important`, etc.) without moving.
    SetFlag { flag: String },
    /// Multiple actions in order.
    Sequence { actions: Vec<Action> },
}

/// Where a rule came from — drives precedence and trust.
///
/// SECURITY: A federated suggestion can never override a `User` rule, and
/// a `Federated` rule cannot promote into INBOX/Important — that
/// invariant is enforced in the suggestion-application path, not just
/// here. See `thundercrab_suggestions::apply_suggestion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleOrigin {
    /// User created or accepted this rule explicitly. Highest trust.
    User,
    /// Shipped with the platform (PlausiDen defaults).
    Platform,
    /// Came from the federated suggestion ledger and was auto-applied
    /// per the user's policy. Lowest trust; can be revoked client-side.
    Federated,
}

/// A sorting rule.
///
/// Mirrors `mail_config::CategoryRule` plus an `origin` field for client
/// trust tracking. Wire format includes `origin` everywhere — the
/// server's Sieve emitter ignores it; only the client honors it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrabRule {
    /// Stable id. Snake_case, no whitespace.
    pub id: String,
    /// Human-readable name.
    pub display_name: String,
    /// Match condition.
    pub when: MatchExpr,
    /// Action(s) when matched.
    pub action: Action,
    /// Higher = wins. Suggested ranges: `100+` user rules, `50–99`
    /// platform defaults, `1–49` federated suggestions.
    pub score: i32,
    /// If `true`, no further rules evaluate after this fires.
    pub stop_on_match: bool,
    /// Provenance.
    pub origin: RuleOrigin,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sanity: an empty `Always`/`FileInto` rule round-trips.
    #[test]
    fn round_trip_minimal() {
        let r = CrabRule {
            id: "t".into(),
            display_name: "T".into(),
            when: MatchExpr::Always,
            action: Action::FileInto {
                folder: "X".into(),
            },
            score: 1,
            stop_on_match: false,
            origin: RuleOrigin::User,
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: CrabRule = serde_json::from_str(&j).unwrap();
        assert_eq!(r, back);
    }

    /// Wire-compat with `mail_config::CategoryRule`: snake_case tags.
    #[test]
    fn wire_format_uses_snake_case_tags() {
        let r = CrabRule {
            id: "promotions_listunsub".into(),
            display_name: "Promotions".into(),
            when: MatchExpr::HasHeader {
                header: "List-Unsubscribe".into(),
            },
            action: Action::FileInto {
                folder: "Promotions".into(),
            },
            score: 80,
            stop_on_match: true,
            origin: RuleOrigin::Platform,
        };
        let j = serde_json::to_string(&r).unwrap();
        // Tag fields must be `kind` and use snake_case so mail-config
        // can deserialize the same payload.
        assert!(j.contains("\"kind\":\"has_header\""));
        assert!(j.contains("\"kind\":\"file_into\""));
        assert!(j.contains("\"origin\":\"platform\""));
    }

    /// A `mail_config::CategoryRule` JSON payload (without `origin`)
    /// should fail deserialization here — `origin` is required. Catching
    /// the failure is what forces callers to translate explicitly.
    #[test]
    fn missing_origin_fails_explicitly() {
        let j = r#"{
            "id":"x","display_name":"X",
            "when":{"kind":"always"},
            "action":{"kind":"file_into","folder":"X"},
            "score":1,"stop_on_match":false
        }"#;
        assert!(serde_json::from_str::<CrabRule>(j).is_err());
    }
}
