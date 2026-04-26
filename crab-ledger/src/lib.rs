//! `crab-ledger` — public, append-only ledger for Thundercrab's
//! federated rule learning.
//!
//! Operators (PlausiDen runs one; anyone can run their own) host this
//! service over Tor (recommended) or clearnet. Clients submit signed
//! `SignedSuggestion`s; the ledger verifies the signature, dedupes by
//! `(pattern_hash, public_key)`, and serves aggregated corroboration
//! counts.
//!
//! ## Privacy properties
//!
//! - **No IP logging.** The Axum service does not read or store
//!   `X-Forwarded-For`, peer addr, or User-Agent. Operators must also
//!   configure their reverse proxy / Tor descriptor to drop logs.
//! - **No precise timestamps.** `submitted_at_day` is days-since-epoch.
//!   The ledger preserves that precision and adds nothing finer.
//! - **No request bodies kept.** Once verified and counted, the raw
//!   submission body is discarded; only `(pattern_hash, public_key,
//!   signature, day)` is retained.
//! - **No content.** The signed body itself can only carry rule
//!   patterns (header-only matches), so even if every public_key were
//!   somehow tied to a user, the on-disk payload reveals nothing about
//!   their mailbox.
//!
//! ## What this is NOT
//!
//! - Not authenticated. There are no accounts. The corroboration count
//!   is the trust signal; reputation/quotas are an operator concern.
//! - Not a CDN. For high-read deployments, snapshot the SQLite read
//!   side and serve via static files behind a cache.

#![doc(html_no_source)]

pub mod api;
pub mod store;

pub use api::router;
pub use store::{Store, StoreError};
