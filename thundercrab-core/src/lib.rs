//! `thundercrab-core` — local rules, flag events, and the wire schema
//! shared with the `mail-config` crate in the PlausiDen mail server.
//!
//! Design principles (per AVP Doctrine):
//!   * Zero unsafe.
//!   * No code path looks at message bodies. Headers + Subject + From
//!     domain only. Anything else is a bug.
//!   * The rule struct is byte-identical between server-side
//!     (`mail_config::CategoryRule`) and client-side use; `crab_rule.rs`
//!     re-declares it with the same serde tags so swapping crate
//!     boundaries is a no-op for callers.
//!
//! Governed by the PlausiDen AVP Doctrine. Every public function carries
//! a `BUG ASSUMPTION:` annotation; every defense-in-depth carries a
//! `SECURITY:` annotation.

#![doc(html_no_source)]

pub mod crab_rule;
pub mod db;
pub mod flag_event;

pub use crab_rule::{Action, CrabRule, MatchExpr, RuleOrigin};
pub use db::{Db, DbError};
pub use flag_event::{FlagEvent, FlagSource};
