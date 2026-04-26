//! `FlagEvent` — record of a user manually moving / flagging a message.
//!
//! These are the raw observations that the suggestion engine derives
//! rules from. Each event references the message only by a content hash
//! and a small set of typed header *features* — never the full headers,
//! never the body.
//!
//! SECURITY: When events are exported to the federated ledger, only the
//! *features* survive (see `FeatureBag`); the content hash and per-user
//! timestamps stay local.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// What the user did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagSource {
    /// User dragged/keyboard-shortcut moved a message into a folder.
    ManualMove,
    /// User toggled the "Important" flag (or equivalent).
    ToggleFlag,
    /// User explicitly hit "This is Promotions/Social/etc." in the UI.
    /// Strongest signal.
    ExplicitCategory,
    /// User accepted a federated suggestion. Useful for closing the
    /// loop on which suggestions are working.
    AcceptSuggestion,
}

/// A user-flagging event recorded in the local SQLite DB.
///
/// `from_domain_with_at` and `subject_tokens` are the *features*
/// extracted from the message at observation time. The full From, full
/// Subject, body, and recipient are NEVER stored here.
///
/// BUG ASSUMPTION: `subject_tokens` is already lowercased and stripped
/// of stop-words by the caller; the DB just stores them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagEvent {
    /// Auto-incrementing local id; `None` before insert.
    pub local_id: Option<i64>,
    /// Per-message stable id (Message-Id header hash, blake3-32). Used
    /// for de-duplication if the user flips a message twice.
    pub message_hash: [u8; 32],
    /// What the user did.
    pub source: FlagSource,
    /// Destination folder (for moves) or flag name (for flag toggles).
    pub destination: String,
    /// Sender domain *with* leading `@`, e.g., `@github.com`. Empty if
    /// not parseable.
    pub from_domain_with_at: String,
    /// Lowercased List-Id value (without angle brackets), if present.
    pub list_id: Option<String>,
    /// `true` if a List-Unsubscribe header was present.
    pub has_list_unsubscribe: bool,
    /// Lowercased subject tokens (stop-words removed), capped at 16.
    pub subject_tokens: Vec<String>,
    /// `X-Priority` value if 1 or 2.
    pub priority_high: bool,
    /// When the flag happened, UTC.
    pub observed_at: DateTime<Utc>,
}

impl FlagEvent {
    /// Hex of the message hash, for logging / debugging only. Never
    /// transmitted off-device.
    #[must_use]
    pub fn message_hash_hex(&self) -> String {
        self.message_hash
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let ev = FlagEvent {
            local_id: None,
            message_hash: [7u8; 32],
            source: FlagSource::ExplicitCategory,
            destination: "Promotions".into(),
            from_domain_with_at: "@mailchimp.com".into(),
            list_id: Some("news.mailchimp.com".into()),
            has_list_unsubscribe: true,
            subject_tokens: vec!["sale".into(), "deals".into()],
            priority_high: false,
            observed_at: Utc::now(),
        };
        let j = serde_json::to_string(&ev).unwrap();
        let back: FlagEvent = serde_json::from_str(&j).unwrap();
        assert_eq!(ev.message_hash, back.message_hash);
        assert_eq!(ev.from_domain_with_at, back.from_domain_with_at);
    }

    #[test]
    fn message_hash_hex_is_64_chars() {
        let ev = FlagEvent {
            local_id: None,
            message_hash: [0xab; 32],
            source: FlagSource::ManualMove,
            destination: "X".into(),
            from_domain_with_at: String::new(),
            list_id: None,
            has_list_unsubscribe: false,
            subject_tokens: vec![],
            priority_high: false,
            observed_at: Utc::now(),
        };
        let h = ev.message_hash_hex();
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
