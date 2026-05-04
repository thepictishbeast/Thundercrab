//! Property-based tests for the federated-suggestion safety guards.
//!
//! Federated rules are adversarial input: anyone on the network can
//! sign + publish a suggestion, and the local client has to refuse
//! the dangerous ones. These tests bound the safety guarantees:
//!
//! 1. **No federated rule ever clamps to score >= 50.** Federated
//!    rules sit below platform defaults (50–99) and explicit user
//!    rules (100+), period.
//! 2. **Origin is always overwritten to Federated.** Even if the
//!    incoming `CrabRule` claims `RuleOrigin::User`, the applied
//!    rule is marked Federated — we don't trust the claim.
//! 3. **Protected folders never receive a federated FileInto.**
//!    Inbox / Important / case-variants always reject.
//! 4. **Protected flags never receive a federated SetFlag.**
//!    \Flagged, $Important, etc. always reject.
//! 5. **Nested actions cannot smuggle through.** A Sequence
//!    containing a protected sub-action rejects with Nested.

use proptest::prelude::*;
use thundercrab_core::{Action, CrabRule, MatchExpr, RuleOrigin};
use thundercrab_suggestions::safety::{SafetyError, apply_suggestion};

/// Generate a `MatchExpr` from the safe-by-construction subset.
fn arb_safe_match() -> impl Strategy<Value = MatchExpr> {
    prop_oneof![
        Just(MatchExpr::Always),
        "[a-z][a-z0-9-]{0,15}".prop_map(|h| MatchExpr::HasHeader { header: h }),
        ("[a-z][a-z0-9-]{0,15}", "[a-zA-Z]{1,20}").prop_map(|(h, s)| {
            MatchExpr::HeaderContains {
                header: h,
                substring: s,
            }
        }),
        prop::collection::vec("@[a-z][a-z0-9.-]{0,15}\\.[a-z]{2,5}", 1..3)
            .prop_map(|d| MatchExpr::FromDomainIn { domains: d }),
        prop::collection::vec("[a-zA-Z]{1,15}", 1..3)
            .prop_map(|n| MatchExpr::SubjectContainsAny { needles: n }),
    ]
}

/// Generate an arbitrary `RuleOrigin` (including User / Platform that
/// would let an unsafe upstream try to bypass clamping).
fn arb_origin() -> impl Strategy<Value = RuleOrigin> {
    prop_oneof![
        Just(RuleOrigin::User),
        Just(RuleOrigin::Platform),
        Just(RuleOrigin::Federated),
    ]
}

fn arb_rule_with_safe_action() -> impl Strategy<Value = CrabRule> {
    (
        "[a-z][a-z0-9_]{2,20}",
        "[A-Za-z][A-Za-z0-9 ]{2,30}",
        arb_safe_match(),
        any::<i32>(),
        any::<bool>(),
        arb_origin(),
        // Folder is anything that is NOT a protected name.
        "[A-Za-z][A-Za-z0-9 _-]{2,20}",
    )
        .prop_filter("avoid protected folders by name", |t| {
            !["INBOX", "Inbox", "inbox", "Important"].contains(&t.6.as_str())
        })
        .prop_map(
            |(id, display_name, when, score, stop, origin, folder)| CrabRule {
                id,
                display_name,
                when,
                action: Action::FileInto { folder },
                score,
                stop_on_match: stop,
                origin,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Successfully-applied rules always carry origin=Federated and
    /// score within 1..=49 inclusive, regardless of input claim.
    #[test]
    fn applied_rules_are_clamped(rule in arb_rule_with_safe_action()) {
        let result = apply_suggestion(&rule);
        if let Ok(applied) = result {
            prop_assert_eq!(applied.origin, RuleOrigin::Federated);
            prop_assert!(applied.score >= 1, "score below floor: {}", applied.score);
            prop_assert!(applied.score <= 49, "score above ceiling: {}", applied.score);
        }
    }

    /// Any FileInto targeting a protected folder name is rejected,
    /// regardless of case or surrounding rule shape.
    #[test]
    fn protected_folders_always_reject(
        when in arb_safe_match(),
        folder in prop::sample::select(vec!["INBOX", "Inbox", "inbox", "iNbOx", "Important", "important", "IMPORTANT"]),
        origin in arb_origin(),
        score in any::<i32>(),
    ) {
        let rule = CrabRule {
            id: "test".into(),
            display_name: "Test".into(),
            when,
            action: Action::FileInto { folder: folder.into() },
            score,
            stop_on_match: false,
            origin,
        };
        let res = apply_suggestion(&rule);
        prop_assert!(matches!(res, Err(SafetyError::ProtectedFolder { .. })), "expected ProtectedFolder for {folder:?}");
    }

    /// Any SetFlag of a protected flag is rejected.
    #[test]
    fn protected_flags_always_reject(
        when in arb_safe_match(),
        flag in prop::sample::select(vec!["\\Flagged", "$Important", "$Label1"]),
        origin in arb_origin(),
        score in any::<i32>(),
    ) {
        let rule = CrabRule {
            id: "test".into(),
            display_name: "Test".into(),
            when,
            action: Action::SetFlag { flag: flag.into() },
            score,
            stop_on_match: false,
            origin,
        };
        let res = apply_suggestion(&rule);
        prop_assert!(matches!(res, Err(SafetyError::ProtectedFlag { .. })), "expected ProtectedFlag for {flag:?}");
    }

    /// A Sequence with one protected sub-action rejects with Nested,
    /// regardless of how many safe sub-actions surround it.
    #[test]
    fn protected_in_sequence_rejects_as_nested(
        safe_folder in "[A-Za-z][A-Za-z0-9 _-]{2,20}",
        bad_folder in prop::sample::select(vec!["INBOX", "Important", "Inbox"]),
    ) {
        prop_assume!(!["INBOX", "Inbox", "inbox", "Important"].contains(&safe_folder.as_str()));
        let rule = CrabRule {
            id: "test".into(),
            display_name: "Test".into(),
            when: MatchExpr::Always,
            action: Action::Sequence {
                actions: vec![
                    Action::FileInto { folder: safe_folder },
                    Action::FileInto { folder: bad_folder.into() },
                ],
            },
            score: 10,
            stop_on_match: false,
            origin: RuleOrigin::User,
        };
        prop_assert!(matches!(apply_suggestion(&rule), Err(SafetyError::Nested(_))));
    }
}
