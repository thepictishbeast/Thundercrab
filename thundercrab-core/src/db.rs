//! SQLite-backed local store for rules and flag events.
//!
//! Storage layout:
//!
//! ```text
//! rules         id TEXT PRIMARY KEY, json TEXT NOT NULL,
//!               origin TEXT NOT NULL, score INTEGER NOT NULL,
//!               updated_at INTEGER NOT NULL
//!
//! flag_events   local_id INTEGER PRIMARY KEY AUTOINCREMENT,
//!               message_hash BLOB NOT NULL,
//!               source TEXT NOT NULL,
//!               destination TEXT NOT NULL,
//!               from_domain_with_at TEXT NOT NULL,
//!               list_id TEXT,
//!               has_list_unsubscribe INTEGER NOT NULL,
//!               subject_tokens_json TEXT NOT NULL,
//!               priority_high INTEGER NOT NULL,
//!               observed_at INTEGER NOT NULL,
//!               UNIQUE(message_hash, source, destination)
//! ```
//!
//! SECURITY: Open the DB file with mode 0600 — flag events are private
//! observations of the user's mailbox even though only features are
//! stored. The caller (the desktop app) sets the permission; the DB
//! layer doesn't fight the umask.

use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use thiserror::Error;

use crate::crab_rule::{CrabRule, RuleOrigin};
use crate::flag_event::{FlagEvent, FlagSource};

/// DB-layer errors.
#[derive(Debug, Error)]
pub enum DbError {
    /// Underlying SQLite error.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// JSON serialization or deserialization failed.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// Schema version on disk is newer than this binary supports.
    #[error("db schema version {found} is newer than supported {supported}")]
    #[allow(missing_docs)]
    SchemaTooNew {
        /// Version recorded in the file.
        found: i64,
        /// Highest version this binary knows how to use.
        supported: i64,
    },
}

/// Result alias for DB ops.
pub type DbResult<T> = Result<T, DbError>;

/// Current schema version.
const SCHEMA_VERSION: i64 = 1;

/// SQLite handle for Thundercrab's local store.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open or create the DB at `path`. Runs migrations to current
    /// schema. Use `:memory:` (literal) for an in-process test DB.
    ///
    /// BUG ASSUMPTION: The caller has already created the parent dir
    /// with appropriate permissions. We do not `mkdir -p` here.
    ///
    /// # Errors
    /// `DbError::Sqlite` for connection / migration failures;
    /// `DbError::SchemaTooNew` when the on-disk schema is newer than
    /// this binary supports.
    pub fn open(path: impl AsRef<Path>) -> DbResult<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// Open an in-memory DB. Test-only convenience.
    ///
    /// # Errors
    /// Same `DbError` variants as [`Self::open`].
    pub fn open_in_memory() -> DbResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> DbResult<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS rules (
                 id TEXT PRIMARY KEY,
                 json TEXT NOT NULL,
                 origin TEXT NOT NULL,
                 score INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS flag_events (
                 local_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 message_hash BLOB NOT NULL,
                 source TEXT NOT NULL,
                 destination TEXT NOT NULL,
                 from_domain_with_at TEXT NOT NULL,
                 list_id TEXT,
                 has_list_unsubscribe INTEGER NOT NULL,
                 subject_tokens_json TEXT NOT NULL,
                 priority_high INTEGER NOT NULL,
                 observed_at INTEGER NOT NULL,
                 UNIQUE(message_hash, source, destination)
             );
             CREATE INDEX IF NOT EXISTS flag_events_dest
                 ON flag_events(destination);
             CREATE INDEX IF NOT EXISTS flag_events_from_dom
                 ON flag_events(from_domain_with_at);",
        )?;
        let found = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .optional()?
            .flatten()
            .unwrap_or(0);
        if found > SCHEMA_VERSION {
            return Err(DbError::SchemaTooNew {
                found,
                supported: SCHEMA_VERSION,
            });
        }
        if found < SCHEMA_VERSION {
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                params![SCHEMA_VERSION],
            )?;
        }
        Ok(Self { conn })
    }

    /// Upsert a rule. The id is the primary key — re-saving an existing
    /// id replaces the row.
    ///
    /// # Errors
    /// `DbError::Json` if the rule fails to serialize;
    /// `DbError::Sqlite` for the underlying upsert.
    pub fn save_rule(&self, rule: &CrabRule) -> DbResult<()> {
        let json = serde_json::to_string(rule)?;
        let origin = match rule.origin {
            RuleOrigin::User => "user",
            RuleOrigin::Platform => "platform",
            RuleOrigin::Federated => "federated",
        };
        self.conn.execute(
            "INSERT INTO rules (id, json, origin, score, updated_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%s','now'))
             ON CONFLICT(id) DO UPDATE SET
                 json = excluded.json,
                 origin = excluded.origin,
                 score = excluded.score,
                 updated_at = excluded.updated_at",
            params![rule.id, json, origin, rule.score],
        )?;
        Ok(())
    }

    /// Load every rule, ordered by score desc then id asc — the same
    /// order `mail_config::CategoryRules::evaluate` uses
    /// (Mailroom-side; not a direct dep here, so written as plain
    /// code span rather than an intra-doc link).
    ///
    /// # Errors
    /// `DbError::Sqlite` for the SELECT; `DbError::Json` for any
    /// stored row that fails to deserialize.
    pub fn load_rules(&self) -> DbResult<Vec<CrabRule>> {
        let mut stmt = self
            .conn
            .prepare("SELECT json FROM rules ORDER BY score DESC, id ASC")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for j in rows {
            out.push(serde_json::from_str(&j)?);
        }
        Ok(out)
    }

    /// Delete a rule by id. Returns `true` if a row was removed.
    ///
    /// # Errors
    /// `DbError::Sqlite` for the underlying DELETE.
    pub fn delete_rule(&self, id: &str) -> DbResult<bool> {
        let n = self
            .conn
            .execute("DELETE FROM rules WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    /// Record a flag event. `UNIQUE(message_hash, source, destination)`
    /// makes repeated calls for the same observation a no-op.
    ///
    /// # Errors
    /// `DbError::Json` if the subject-tokens vector fails to
    /// serialize; `DbError::Sqlite` for the INSERT.
    pub fn record_flag(&self, ev: &FlagEvent) -> DbResult<()> {
        let source = match ev.source {
            FlagSource::ManualMove => "manual_move",
            FlagSource::ToggleFlag => "toggle_flag",
            FlagSource::ExplicitCategory => "explicit_category",
            FlagSource::AcceptSuggestion => "accept_suggestion",
        };
        let tokens = serde_json::to_string(&ev.subject_tokens)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO flag_events
                (message_hash, source, destination, from_domain_with_at,
                 list_id, has_list_unsubscribe, subject_tokens_json,
                 priority_high, observed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &ev.message_hash[..],
                source,
                ev.destination,
                ev.from_domain_with_at,
                ev.list_id,
                i64::from(ev.has_list_unsubscribe),
                tokens,
                i64::from(ev.priority_high),
                ev.observed_at.timestamp(),
            ],
        )?;
        Ok(())
    }

    /// Count how many flag events route to a given destination — input
    /// for the suggestion-derivation algorithm.
    ///
    /// # Errors
    /// `DbError::Sqlite` for the SELECT.
    pub fn count_flags_to(&self, destination: &str) -> DbResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM flag_events WHERE destination = ?1",
            params![destination],
            |r| r.get(0),
        )?)
    }

    /// Count flag events grouped by `(from_domain_with_at, destination)`
    /// — feeds the "users who move @x.com to Y consistently" derivation.
    ///
    /// # Errors
    /// `DbError::Sqlite` for the GROUP BY query or row decoding.
    pub fn flag_counts_by_domain_dest(&self) -> DbResult<Vec<(String, String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT from_domain_with_at, destination, COUNT(*)
             FROM flag_events
             WHERE from_domain_with_at <> ''
             GROUP BY from_domain_with_at, destination
             ORDER BY 3 DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crab_rule::{Action, MatchExpr};
    use chrono::Utc;

    fn ev(dest: &str, dom: &str, h: u8) -> FlagEvent {
        FlagEvent {
            local_id: None,
            message_hash: [h; 32],
            source: FlagSource::ManualMove,
            destination: dest.into(),
            from_domain_with_at: dom.into(),
            list_id: None,
            has_list_unsubscribe: false,
            subject_tokens: vec![],
            priority_high: false,
            observed_at: Utc::now(),
        }
    }

    #[test]
    fn rules_round_trip_via_sqlite() {
        let db = Db::open_in_memory().unwrap();
        let r = CrabRule {
            id: "x".into(),
            display_name: "x".into(),
            when: MatchExpr::HasHeader {
                header: "List-Id".into(),
            },
            action: Action::FileInto {
                folder: "Forums".into(),
            },
            score: 75,
            stop_on_match: true,
            origin: RuleOrigin::User,
        };
        db.save_rule(&r).unwrap();
        let loaded = db.load_rules().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0], r);
    }

    #[test]
    fn rules_ordered_by_score_desc() {
        let db = Db::open_in_memory().unwrap();
        for (id, score) in [("low", 10), ("high", 90), ("mid", 50)] {
            db.save_rule(&CrabRule {
                id: id.into(),
                display_name: id.into(),
                when: MatchExpr::Always,
                action: Action::FileInto { folder: "X".into() },
                score,
                stop_on_match: false,
                origin: RuleOrigin::Platform,
            })
            .unwrap();
        }
        let loaded = db.load_rules().unwrap();
        let ids: Vec<_> = loaded.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["high", "mid", "low"]);
    }

    #[test]
    fn duplicate_flag_event_is_ignored() {
        let db = Db::open_in_memory().unwrap();
        db.record_flag(&ev("Promotions", "@x.com", 1)).unwrap();
        db.record_flag(&ev("Promotions", "@x.com", 1)).unwrap();
        assert_eq!(db.count_flags_to("Promotions").unwrap(), 1);
    }

    #[test]
    fn flag_counts_by_domain_dest_groups_correctly() {
        let db = Db::open_in_memory().unwrap();
        db.record_flag(&ev("Promotions", "@a.com", 1)).unwrap();
        db.record_flag(&ev("Promotions", "@a.com", 2)).unwrap();
        db.record_flag(&ev("Promotions", "@b.com", 3)).unwrap();
        db.record_flag(&ev("Social", "@a.com", 4)).unwrap();
        let mut counts = db.flag_counts_by_domain_dest().unwrap();
        counts.sort();
        assert_eq!(
            counts,
            vec![
                ("@a.com".into(), "Promotions".into(), 2),
                ("@a.com".into(), "Social".into(), 1),
                ("@b.com".into(), "Promotions".into(), 1),
            ]
        );
    }

    #[test]
    fn delete_rule_returns_true_then_false() {
        let db = Db::open_in_memory().unwrap();
        db.save_rule(&CrabRule {
            id: "x".into(),
            display_name: "x".into(),
            when: MatchExpr::Always,
            action: Action::FileInto { folder: "X".into() },
            score: 1,
            stop_on_match: false,
            origin: RuleOrigin::User,
        })
        .unwrap();
        assert!(db.delete_rule("x").unwrap());
        assert!(!db.delete_rule("x").unwrap());
    }
}
