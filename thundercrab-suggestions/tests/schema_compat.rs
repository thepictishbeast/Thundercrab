//! Wire-format compatibility test between
//! `mail_config::CategoryRule` (orchestrator side) and
//! `thundercrab_core::CrabRule` (client side).
//!
//! We can't depend on `mail-config` directly without making the
//! Secure-Email repo a workspace dep, so we pin a fixture: a JSON blob
//! that `mail-config` is documented to emit, plus the `origin` field
//! ThunderCrab needs. Round-trip it and assert structural identity.
//!
//! If this test breaks, **either** `mail-config::CategoryRule` got a
//! new field that wasn't mirrored here, **or** `CrabRule`'s serde tags
//! drifted. Either way, fix it before merging.

use thundercrab_core::{Action, CrabRule, MatchExpr, RuleOrigin};

const FIXTURE_FROM_MAIL_CONFIG_PLUS_ORIGIN: &str = r#"{
  "id": "promotions_listunsub",
  "display_name": "Promotions — List-Unsubscribe present",
  "when": { "kind": "has_header", "header": "List-Unsubscribe" },
  "action": { "kind": "file_into", "folder": "Promotions" },
  "score": 80,
  "stop_on_match": true,
  "origin": "platform"
}"#;

#[test]
fn mail_config_rule_with_origin_deserializes_into_crab_rule() {
    let r: CrabRule = serde_json::from_str(FIXTURE_FROM_MAIL_CONFIG_PLUS_ORIGIN).unwrap();
    assert_eq!(r.id, "promotions_listunsub");
    assert_eq!(r.score, 80);
    assert!(r.stop_on_match);
    assert_eq!(r.origin, RuleOrigin::Platform);
    assert_eq!(
        r.when,
        MatchExpr::HasHeader {
            header: "List-Unsubscribe".into(),
        }
    );
    assert_eq!(
        r.action,
        Action::FileInto {
            folder: "Promotions".into(),
        }
    );
}

#[test]
fn nested_match_expr_round_trips() {
    // Sanity: the nested AST shapes used by the mail-config defaults
    // (X-Priority OR with two header-contains) survive round-trip.
    let fixture = r#"{
      "id": "important_priority",
      "display_name": "Important",
      "when": {
        "kind": "any",
        "exprs": [
          {"kind":"header_contains","header":"X-Priority","substring":"1"},
          {"kind":"header_contains","header":"X-Priority","substring":"2"}
        ]
      },
      "action": {"kind":"set_flag","flag":"\\Flagged"},
      "score": 90,
      "stop_on_match": false,
      "origin": "platform"
    }"#;
    let r: CrabRule = serde_json::from_str(fixture).unwrap();
    let back = serde_json::to_string(&r).unwrap();
    let r2: CrabRule = serde_json::from_str(&back).unwrap();
    assert_eq!(r, r2);
}
