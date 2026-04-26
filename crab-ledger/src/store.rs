//! SQLite-backed corroboration store.
//!
//! Schema:
//!
//! ```text
//! corroborations (
//!     pattern_hash    BLOB NOT NULL,
//!     public_key      BLOB NOT NULL,
//!     signature       BLOB NOT NULL,
//!     rule_json       TEXT NOT NULL,        -- canonical rule body
//!     submitted_day   INTEGER NOT NULL,
//!     PRIMARY KEY (pattern_hash, public_key)
//! );
//!
//! CREATE INDEX corroborations_pattern ON corroborations(pattern_hash);
//! CREATE INDEX corroborations_day ON corroborations(submitted_day);
//! ```
//!
//! `(pattern_hash, public_key)` is the dedup key — a single install
//! re-submitting the same rule overwrites its row, never inflates count.
//!
//! SECURITY: We store the canonical `rule_json` so reads can return the
//! rule shape without re-canonicalizing on every fetch. Verifying that
//! `pattern_hash == blake3(rule_json)` is done at submit time
//! (`thundercrab_suggestions::ledger::verify`) — readers trust the
//! ledger's prior verification.

use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use thiserror::Error;

/// Errors from the store layer.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Underlying SQLite error.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// JSON encode/decode error.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// One ledger entry as the store sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorroborationRow {
    /// blake3-32 of the canonical rule body.
    pub pattern_hash: [u8; 32],
    /// Ed25519 public key of the submitter.
    pub public_key: [u8; 32],
    /// Ed25519 signature.
    pub signature: [u8; 64],
    /// Canonical rule JSON.
    pub rule_json: String,
    /// Days since Unix epoch when this corroboration was first seen.
    pub submitted_day: i64,
}

/// One row of `/v1/suggestions` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregatedSuggestion {
    /// blake3-32 hex of the rule body.
    pub pattern_hash_hex: String,
    /// One canonical copy of the rule JSON. Any corroborator's body
    /// works; we pick the lexicographically smallest for determinism.
    pub rule_json: String,
    /// Distinct public-key count.
    pub corroborators: i64,
    /// Earliest `submitted_day` seen for this `pattern_hash`.
    pub first_seen_day: i64,
}

/// Connection-owning store handle.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open or create the SQLite store at `path`. Use `:memory:` for
    /// an in-process test store.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// Open an in-memory store. Test-only.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS corroborations (
                 pattern_hash    BLOB NOT NULL,
                 public_key      BLOB NOT NULL,
                 signature       BLOB NOT NULL,
                 rule_json       TEXT NOT NULL,
                 submitted_day   INTEGER NOT NULL,
                 PRIMARY KEY (pattern_hash, public_key)
             );
             CREATE INDEX IF NOT EXISTS corroborations_pattern
                 ON corroborations(pattern_hash);
             CREATE INDEX IF NOT EXISTS corroborations_day
                 ON corroborations(submitted_day);",
        )?;
        Ok(Self { conn })
    }

    /// Upsert a corroboration row.
    ///
    /// Returns `true` if this is a new corroborator for `pattern_hash`
    /// (i.e., a row was inserted, not just updated). Useful for the
    /// `/v1/submit` endpoint to honestly report "newly counted" vs
    /// "duplicate".
    pub fn upsert(&self, row: &CorroborationRow) -> Result<bool, StoreError> {
        let existed: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM corroborations WHERE pattern_hash = ?1 AND public_key = ?2",
                params![&row.pattern_hash[..], &row.public_key[..]],
                |_| Ok(true),
            )
            .optional()?
            .is_some();

        self.conn.execute(
            "INSERT INTO corroborations
                 (pattern_hash, public_key, signature, rule_json, submitted_day)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(pattern_hash, public_key) DO UPDATE SET
                 signature = excluded.signature,
                 rule_json = excluded.rule_json,
                 submitted_day = MIN(corroborations.submitted_day, excluded.submitted_day)",
            params![
                &row.pattern_hash[..],
                &row.public_key[..],
                &row.signature[..],
                row.rule_json,
                row.submitted_day,
            ],
        )?;
        Ok(!existed)
    }

    /// Aggregate corroboration counts across all `pattern_hash`es,
    /// optionally filtered to those at or above `min_corroborators` and
    /// first-seen at or after `since_day`.
    ///
    /// Sorted by corroborator count descending then `pattern_hash` for
    /// deterministic output.
    pub fn list_aggregated(
        &self,
        min_corroborators: i64,
        since_day: Option<i64>,
    ) -> Result<Vec<AggregatedSuggestion>, StoreError> {
        let since = since_day.unwrap_or(i64::MIN);
        let mut stmt = self.conn.prepare(
            "SELECT
                 pattern_hash,
                 MIN(rule_json) AS rule_json,
                 COUNT(DISTINCT public_key) AS corroborators,
                 MIN(submitted_day) AS first_seen_day
             FROM corroborations
             GROUP BY pattern_hash
             HAVING corroborators >= ?1 AND first_seen_day >= ?2
             ORDER BY corroborators DESC, pattern_hash ASC",
        )?;
        let rows = stmt.query_map(params![min_corroborators, since], |r| {
            let pattern_hash: Vec<u8> = r.get(0)?;
            Ok(AggregatedSuggestion {
                pattern_hash_hex: pattern_hash.iter().map(|b| format!("{b:02x}")).collect(),
                rule_json: r.get(1)?,
                corroborators: r.get(2)?,
                first_seen_day: r.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Total distinct pattern_hash count — for `/v1/health`.
    pub fn total_unique_patterns(&self) -> Result<i64, StoreError> {
        Ok(self.conn.query_row(
            "SELECT COUNT(DISTINCT pattern_hash) FROM corroborations",
            [],
            |r| r.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(ph: [u8; 32], pk: [u8; 32], rule_json: &str, day: i64) -> CorroborationRow {
        CorroborationRow {
            pattern_hash: ph,
            public_key: pk,
            signature: [9u8; 64],
            rule_json: rule_json.into(),
            submitted_day: day,
        }
    }

    #[test]
    fn upsert_returns_true_on_new_pubkey() {
        let s = Store::open_in_memory().unwrap();
        assert!(s.upsert(&row([1; 32], [2; 32], "{}", 100)).unwrap());
        // Same pattern, same key → not new
        assert!(!s.upsert(&row([1; 32], [2; 32], "{}", 101)).unwrap());
        // Same pattern, different key → new
        assert!(s.upsert(&row([1; 32], [3; 32], "{}", 102)).unwrap());
    }

    #[test]
    fn aggregation_counts_distinct_pubkeys() {
        let s = Store::open_in_memory().unwrap();
        // Three keys vote for pattern A, one votes for pattern B.
        s.upsert(&row([1; 32], [10; 32], "{\"id\":\"A\"}", 100)).unwrap();
        s.upsert(&row([1; 32], [11; 32], "{\"id\":\"A\"}", 101)).unwrap();
        s.upsert(&row([1; 32], [12; 32], "{\"id\":\"A\"}", 102)).unwrap();
        s.upsert(&row([2; 32], [10; 32], "{\"id\":\"B\"}", 200)).unwrap();

        let all = s.list_aggregated(1, None).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].corroborators, 3);
        assert_eq!(all[0].first_seen_day, 100);
        assert_eq!(all[1].corroborators, 1);
    }

    #[test]
    fn aggregation_min_corroborators_filter() {
        let s = Store::open_in_memory().unwrap();
        s.upsert(&row([1; 32], [10; 32], "{}", 100)).unwrap();
        s.upsert(&row([1; 32], [11; 32], "{}", 101)).unwrap();
        s.upsert(&row([2; 32], [10; 32], "{}", 200)).unwrap();
        // Pattern 1: 2 corroborators. Pattern 2: 1.
        let filtered = s.list_aggregated(2, None).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].corroborators, 2);
    }

    #[test]
    fn aggregation_since_day_filter() {
        let s = Store::open_in_memory().unwrap();
        s.upsert(&row([1; 32], [10; 32], "{}", 100)).unwrap();
        s.upsert(&row([2; 32], [11; 32], "{}", 200)).unwrap();
        let recent = s.list_aggregated(1, Some(150)).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].first_seen_day, 200);
    }

    #[test]
    fn duplicate_submission_keeps_earliest_day() {
        let s = Store::open_in_memory().unwrap();
        s.upsert(&row([1; 32], [10; 32], "{}", 200)).unwrap();
        s.upsert(&row([1; 32], [10; 32], "{}", 100)).unwrap();
        let all = s.list_aggregated(1, None).unwrap();
        assert_eq!(all[0].first_seen_day, 100);
    }

    #[test]
    fn total_unique_patterns_reports_distinct_hashes() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.total_unique_patterns().unwrap(), 0);
        s.upsert(&row([1; 32], [10; 32], "{}", 100)).unwrap();
        s.upsert(&row([1; 32], [11; 32], "{}", 100)).unwrap();
        s.upsert(&row([2; 32], [10; 32], "{}", 100)).unwrap();
        assert_eq!(s.total_unique_patterns().unwrap(), 2);
    }
}
