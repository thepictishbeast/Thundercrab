//! Derive candidate rules from local flag events.
//!
//! Approach: for each `(from_domain, destination)` pair where the user
//! has consistently moved messages, propose a rule
//! `FromDomainIn([@domain]) → FileInto(destination)`. Only emit if:
//!   * count ≥ `min_observations`, and
//!   * the destination is the *dominant* destination for that domain
//!     (i.e., `> dominance_threshold` of all flag events for the
//!     domain), so we don't propose conflicting rules.
//!
//! This is intentionally simple. Better candidate-derivation algorithms
//! (List-Id-driven, subject-token-driven, multi-feature) are tracked in
//! `docs/SUGGESTION_HEURISTICS.md` as future work.

use thundercrab_core::{Action, CrabRule, MatchExpr, RuleOrigin};

/// A rule the local install proposes, both for self-application and for
/// optional submission to the federated ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedRule {
    /// The proposed rule. Always [`RuleOrigin::User`] at this point —
    /// the federated path re-marks it [`RuleOrigin::Federated`] when
    /// re-imported via [`super::apply_suggestion`].
    pub rule: CrabRule,
    /// How many local flag events backed this rule.
    pub local_evidence_count: i64,
}

/// Derive rule candidates from a domain×destination grouping.
///
/// `flag_counts` is the output of
/// [`thundercrab_core::Db::flag_counts_by_domain_dest`].
///
/// `min_observations` — drop pairs below this count. Default 3 in
/// production.
///
/// `dominance_threshold` — fraction in `[0.0, 1.0]`. The destination
/// must account for at least this fraction of total events for the
/// domain. Default 0.7 (i.e., 70%).
///
/// BUG ASSUMPTION: `flag_counts` may contain the same `(domain,
/// destination)` row only once. The grouped SQL query in
/// `Db::flag_counts_by_domain_dest` guarantees this; if you call from
/// elsewhere, dedupe first.
#[must_use]
pub fn derive_rule_candidates(
    flag_counts: &[(String, String, i64)],
    min_observations: i64,
    dominance_threshold: f64,
) -> Vec<DerivedRule> {
    use std::collections::HashMap;

    // Sum per-domain totals so we can compute dominance.
    let mut totals: HashMap<&str, i64> = HashMap::new();
    for (domain, _, count) in flag_counts {
        *totals.entry(domain.as_str()).or_insert(0) += count;
    }

    let mut out = Vec::new();
    for (domain, dest, count) in flag_counts {
        if *count < min_observations {
            continue;
        }
        let total = *totals.get(domain.as_str()).unwrap_or(&0);
        if total == 0 {
            continue;
        }
        #[allow(clippy::cast_precision_loss)]
        let frac = *count as f64 / total as f64;
        if frac < dominance_threshold {
            continue;
        }

        let id = format!("derived_{}_to_{}", sanitize_id(domain), sanitize_id(dest));
        let rule = CrabRule {
            id,
            display_name: format!("{dest} (derived from {domain})"),
            when: MatchExpr::FromDomainIn {
                domains: vec![domain.clone()],
            },
            action: Action::FileInto {
                folder: dest.clone(),
            },
            // 30 = mid-band federated score; the safety layer clamps
            // to [1, 49] when the rule is re-imported as Federated.
            score: 30,
            stop_on_match: true,
            origin: RuleOrigin::User,
        };
        out.push(DerivedRule {
            rule,
            local_evidence_count: *count,
        });
    }
    out
}

/// Lowercase, replace anything outside `[a-z0-9]` with `_`. Stable
/// across runs so the same `(domain, dest)` always derives the same id
/// — important for federated dedup.
fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominance_threshold_filters_mixed_signal() {
        // @ambig.com gets 4 to A, 6 to B — neither hits 70% dominance.
        let counts = vec![
            ("@ambig.com".into(), "A".into(), 4),
            ("@ambig.com".into(), "B".into(), 6),
        ];
        let derived = derive_rule_candidates(&counts, 3, 0.7);
        assert!(derived.is_empty(), "got {derived:?}");
    }

    #[test]
    fn clear_signal_produces_rule() {
        // @clear.com: 9 to Promotions, 1 to Updates → 90% dominance.
        let counts = vec![
            ("@clear.com".into(), "Promotions".into(), 9),
            ("@clear.com".into(), "Updates".into(), 1),
        ];
        let derived = derive_rule_candidates(&counts, 3, 0.7);
        assert_eq!(derived.len(), 1);
        let r = &derived[0];
        assert_eq!(r.local_evidence_count, 9);
        assert_eq!(r.rule.id, "derived__clear_com_to_promotions");
        assert_eq!(
            r.rule.action,
            Action::FileInto {
                folder: "Promotions".into()
            }
        );
        assert_eq!(
            r.rule.when,
            MatchExpr::FromDomainIn {
                domains: vec!["@clear.com".into()]
            }
        );
    }

    #[test]
    fn min_observations_filter() {
        let counts = vec![("@x.com".into(), "Promotions".into(), 2)];
        let derived = derive_rule_candidates(&counts, 3, 0.7);
        assert!(derived.is_empty());
    }

    #[test]
    fn id_is_deterministic() {
        let counts = vec![("@GitHub.com".into(), "Updates".into(), 5)];
        let a = derive_rule_candidates(&counts, 1, 0.5);
        let b = derive_rule_candidates(&counts, 1, 0.5);
        assert_eq!(a[0].rule.id, b[0].rule.id);
        assert_eq!(a[0].rule.id, "derived__github_com_to_updates");
    }

    #[test]
    fn empty_input_empty_output() {
        let derived = derive_rule_candidates(&[], 3, 0.7);
        assert!(derived.is_empty());
    }
}
