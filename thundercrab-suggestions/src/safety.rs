//! Safety guards for federated suggestions.
//!
//! Two checks live here:
//!   1. [`is_safe_match`] — does a `MatchExpr` only look at allowed
//!      header features?
//!   2. [`apply_suggestion`] — would applying this rule violate the
//!      "no promote into INBOX/Important, no flag-setting" rule?

use thiserror::Error;
use thundercrab_core::{Action, CrabRule, MatchExpr, RuleOrigin};

/// Why a federated suggestion was rejected.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SafetyError {
    /// The match expression references something that should not be
    /// part of a federated rule (e.g., a hypothetical body variant in
    /// a future schema). Today this branch is structurally unreachable
    /// — kept as a defense-in-depth tripwire.
    #[error("match expression contains disallowed feature")]
    UnsafeMatch,

    /// The action targets `INBOX` or a special-use folder we never
    /// allow federated rules to write into.
    #[error("action targets protected folder: {folder}")]
    #[allow(missing_docs)]
    ProtectedFolder {
        /// Folder name the rejected action targeted.
        folder: String,
    },

    /// The action sets a high-importance flag.
    #[error("action sets a protected flag: {flag}")]
    #[allow(missing_docs)]
    ProtectedFlag {
        /// Flag the rejected action would have set.
        flag: String,
    },

    /// A `Sequence` action contains a sub-action that violates one of
    /// the above; reported with the inner failure.
    #[error("nested action violation: {0}")]
    Nested(Box<SafetyError>),
}

/// Folders federated rules cannot file *into*. These are folders the
/// user explicitly cares about being authoritative.
const PROTECTED_FOLDERS: &[&str] = &["INBOX", "Inbox", "Important"];

/// Flags federated rules cannot set. Setting these would let a
/// suggestion promote a message's perceived importance.
const PROTECTED_FLAGS: &[&str] = &["\\Flagged", "$Important", "$Label1"];

/// True if a `MatchExpr` only references allowed header features.
///
/// Today every `MatchExpr` variant is structurally safe by virtue of
/// the AST's design — the type system has no "body" or "full address"
/// variant. This function exists to fail closed if a future variant
/// is added without an audit: anyone adding a new variant must extend
/// this match to mark it allowed.
#[must_use]
pub fn is_safe_match(expr: &MatchExpr) -> bool {
    match expr {
        MatchExpr::Always
        | MatchExpr::HasHeader { .. }
        | MatchExpr::HeaderContains { .. }
        | MatchExpr::FromDomainIn { .. }
        | MatchExpr::SubjectContainsAny { .. } => true,
        MatchExpr::All { exprs } | MatchExpr::Any { exprs } => {
            exprs.iter().all(is_safe_match)
        }
        MatchExpr::Not { expr } => is_safe_match(expr),
    }
}

/// Validate a federated rule and, if it passes, return a copy with
/// `origin = RuleOrigin::Federated` and `score` clamped to `1..=49`.
///
/// SECURITY: This is the only sanctioned path for a federated rule to
/// enter the local store. Callers should never persist a `RuleOrigin::
/// Federated` rule that did not come through here — the `Db` layer
/// trusts what it gets.
pub fn apply_suggestion(rule: &CrabRule) -> Result<CrabRule, SafetyError> {
    if !is_safe_match(&rule.when) {
        return Err(SafetyError::UnsafeMatch);
    }
    check_action(&rule.action)?;
    let score = rule.score.clamp(1, 49);
    Ok(CrabRule {
        origin: RuleOrigin::Federated,
        score,
        ..rule.clone()
    })
}

fn check_action(action: &Action) -> Result<(), SafetyError> {
    match action {
        Action::FileInto { folder } => {
            if PROTECTED_FOLDERS
                .iter()
                .any(|p| p.eq_ignore_ascii_case(folder))
            {
                return Err(SafetyError::ProtectedFolder {
                    folder: folder.clone(),
                });
            }
            Ok(())
        }
        Action::SetFlag { flag } => {
            if PROTECTED_FLAGS.iter().any(|p| *p == flag.as_str()) {
                return Err(SafetyError::ProtectedFlag { flag: flag.clone() });
            }
            Ok(())
        }
        Action::Sequence { actions } => {
            for inner in actions {
                if let Err(e) = check_action(inner) {
                    return Err(SafetyError::Nested(Box::new(e)));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(action: Action) -> CrabRule {
        CrabRule {
            id: "t".into(),
            display_name: "T".into(),
            when: MatchExpr::HasHeader {
                header: "List-Unsubscribe".into(),
            },
            action,
            score: 1000, // intentionally too high; should clamp
            stop_on_match: true,
            origin: RuleOrigin::User,
        }
    }

    #[test]
    fn safe_rule_clamps_score_and_marks_federated() {
        let r = rule(Action::FileInto {
            folder: "Promotions".into(),
        });
        let applied = apply_suggestion(&r).unwrap();
        assert_eq!(applied.origin, RuleOrigin::Federated);
        assert_eq!(applied.score, 49);
    }

    #[test]
    fn negative_score_clamps_to_one() {
        let mut r = rule(Action::FileInto {
            folder: "Promotions".into(),
        });
        r.score = -10;
        let applied = apply_suggestion(&r).unwrap();
        assert_eq!(applied.score, 1);
    }

    #[test]
    fn fileinto_inbox_rejected() {
        let r = rule(Action::FileInto {
            folder: "INBOX".into(),
        });
        assert_eq!(
            apply_suggestion(&r),
            Err(SafetyError::ProtectedFolder {
                folder: "INBOX".into()
            })
        );
    }

    #[test]
    fn fileinto_inbox_case_insensitive() {
        let r = rule(Action::FileInto {
            folder: "inbox".into(),
        });
        assert!(matches!(
            apply_suggestion(&r),
            Err(SafetyError::ProtectedFolder { .. })
        ));
    }

    #[test]
    fn fileinto_important_rejected() {
        let r = rule(Action::FileInto {
            folder: "Important".into(),
        });
        assert!(matches!(
            apply_suggestion(&r),
            Err(SafetyError::ProtectedFolder { .. })
        ));
    }

    #[test]
    fn flagging_flagged_rejected() {
        let r = rule(Action::SetFlag {
            flag: "\\Flagged".into(),
        });
        assert!(matches!(
            apply_suggestion(&r),
            Err(SafetyError::ProtectedFlag { .. })
        ));
    }

    #[test]
    fn nested_protected_action_rejected() {
        let r = rule(Action::Sequence {
            actions: vec![
                Action::FileInto {
                    folder: "Updates".into(),
                },
                Action::FileInto {
                    folder: "INBOX".into(),
                },
            ],
        });
        assert!(matches!(apply_suggestion(&r), Err(SafetyError::Nested(_))));
    }

    #[test]
    fn safe_match_accepts_all_current_variants() {
        for expr in [
            MatchExpr::Always,
            MatchExpr::HasHeader {
                header: "X".into(),
            },
            MatchExpr::HeaderContains {
                header: "X".into(),
                substring: "y".into(),
            },
            MatchExpr::FromDomainIn {
                domains: vec!["@x.com".into()],
            },
            MatchExpr::SubjectContainsAny {
                needles: vec!["x".into()],
            },
        ] {
            assert!(is_safe_match(&expr));
        }
    }
}
