//! `thundercrab-suggestions` — federated rule learning.
//!
//! ## What this is
//!
//! Every Thundercrab install observes its user's flag events
//! ([`thundercrab_core::FlagEvent`]) and *derives* candidate rules from
//! patterns in those events — "this user moves @mailchimp.com → Promotions
//! every time" becomes a candidate rule.
//!
//! Candidate rules are applied locally immediately. They are also,
//! optionally and per-user-consent, signed with the install's keypair
//! and submitted to a public corroboration ledger. Other installs pull
//! from the ledger; rules that N independent installs corroborate become
//! *suggestions* surfaced in the UI for one-click acceptance.
//!
//! ## What this is NOT
//!
//! - This is **not** machine learning. There's no model. Rules are
//!   typed AST patterns; corroboration is by-pattern hash count.
//! - The ledger never sees message content, full sender addresses,
//!   recipients, or timestamps from any user.
//!
//! ## SECURITY invariants enforced here (not advisory)
//!
//! 1. [`derive::derive_rule_candidates`] only returns rules whose
//!    [`thundercrab_core::MatchExpr`] passes [`safety::is_safe_match`]:
//!    no body access (the AST has no body variant — type system enforces),
//!    no full-address matching (only domain-with-at), no regex.
//! 2. [`apply_suggestion`] rejects any suggestion whose action would
//!    move messages **into** `INBOX` or **into** `Important`, or set
//!    the `\Flagged` / `$Important` flag. Only sort-out actions are
//!    allowed via the federated path. (User can still manually create
//!    such rules locally.)
//! 3. Submitted suggestions are signed with the install's per-device
//!    Ed25519 keypair. The ledger validates signatures and treats one
//!    signature = one corroboration vote. Rate-limit and reputation
//!    enforcement live on the ledger side; this crate is the client.

#![doc(html_no_source)]

pub mod derive;
pub mod ledger;
pub mod safety;

pub use derive::{DerivedRule, derive_rule_candidates};
pub use ledger::{InstallKey, SignedSuggestion, Suggestion};
pub use safety::{SafetyError, apply_suggestion, is_safe_match};
