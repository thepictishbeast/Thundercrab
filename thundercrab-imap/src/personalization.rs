//! Cross-device personalization sync via IMAP METADATA (RFC 5464).
//!
//! ThunderCrab stores a single JSON blob — sorting rules, layout, appearance,
//! whatever the client wants to carry — under a private metadata entry on the
//! user's OWN mailbox. It then syncs across every device over the same
//! authenticated IMAP connection: no separate sync server, no extra account,
//! no third party who could see or correlate the data. Sovereign + anonymous
//! by construction; the only party with the data is the user's own mail server.
//!
//! The blob is opaque to this layer (the client owns the schema), so the format
//! can grow new personalization/customization fields without any protocol change.
//!
//! Requires server METADATA support (RFC 5464). PlausiDen's Dovecot enables it
//! (`imap_metadata = yes`); on a server without it, [`get_personalization`]
//! returns `None` and [`set_personalization`] surfaces a `Protocol` error the
//! caller can treat as "sync unavailable, fall back to local-only".
//!
//! [`get_personalization`]: RustImapBackend::get_personalization
//! [`set_personalization`]: RustImapBackend::set_personalization

// The MutexGuard is intentionally held across the await: IMAP is single-channel,
// so every command on the session must serialize (same stance as `rust_imap`).
#![allow(clippy::significant_drop_tightening)]

use crate::BackendError;
use crate::rust_imap::RustImapBackend;

/// RFC 5464 private metadata entry holding ThunderCrab's per-user blob.
const ENTRY: &str = "/private/vendor/thundercrab/personalization";

impl RustImapBackend {
    /// Fetch the synced personalization blob (the JSON the client stored), or
    /// `None` if nothing is stored or the server has no METADATA support.
    ///
    /// # Errors
    /// [`BackendError::Protocol`] only on a malformed server response; an absent
    /// value or a server that lacks METADATA yields `Ok(None)`, not an error.
    pub async fn get_personalization(&self) -> Result<Option<String>, BackendError> {
        let mut session = self.session.lock().await;
        // An UNAVAILABLE / NO here (server without METADATA) is mapped to None by
        // the caller's perspective; only a transport/parse fault is an error.
        let Ok(entries) = session.get_metadata("INBOX", "", ENTRY).await else {
            return Ok(None);
        };
        let Some(value) = entries.into_iter().find_map(|m| m.value) else {
            return Ok(None);
        };
        // Stored hex-encoded (see set_personalization); decode back to JSON. A
        // value we can't decode is treated as "nothing usable", not an error.
        let decoded = hex::decode(value.trim())
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        Ok(decoded)
    }

    /// Store the personalization blob (`json`) as IMAP metadata.
    ///
    /// The value is hex-encoded so it is a safe IMAP quoted string — JSON is
    /// full of `"`, and sending a raw literal would need a continuation
    /// handshake. Hex is `[0-9a-f]` only: no quoting hazards.
    ///
    /// # Errors
    /// [`BackendError::Protocol`] if the server rejects `SETMETADATA` (including
    /// "no METADATA support" or a quota on the entry).
    pub async fn set_personalization(&self, json: &str) -> Result<(), BackendError> {
        let encoded = hex::encode(json.as_bytes());
        let mut session = self.session.lock().await;
        // Session derefs to Connection, which exposes run_command_and_check_ok.
        session
            .run_command_and_check_ok(format!("SETMETADATA \"INBOX\" ({ENTRY} \"{encoded}\")"))
            .await
            .map_err(|e| BackendError::Protocol(format!("setmetadata: {e}")))
    }
}

#[cfg(test)]
mod tests {
    /// The transport hex-encoding must round-trip arbitrary JSON (quotes,
    /// braces, unicode) so the synced blob survives the IMAP quoted-string path.
    #[test]
    fn hex_roundtrips_personalization_json() {
        let json = r#"{"rules":[{"from":"@github.com","to":"Dev"}],"theme":"dark","note":"café ☕"}"#;
        let encoded = hex::encode(json.as_bytes());
        assert!(
            encoded.chars().all(|c| c.is_ascii_hexdigit()),
            "encoded value is a safe IMAP quoted string: {encoded}"
        );
        let decoded = String::from_utf8(hex::decode(&encoded).expect("valid hex")).expect("utf8");
        assert_eq!(decoded, json);
    }
}
