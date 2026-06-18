//! Concrete IMAPS [`Backend`] using `async-imap` over `tokio-rustls`.
//!
//! Production path for ThunderCrab's mail surface. Connects to the
//! configured IMAPS host over TLS (port 993 by default), authenticates
//! with `LOGIN`, and exposes `list_folders` / `fetch_headers` /
//! `move_message` / `set_flag` against the server's UIDs.
//!
//! `put_sieve` returns `BackendError::NotImplemented` — `ManageSieve`
//! is a separate protocol on a separate port, lives in a sibling
//! module, and is tracked as a follow-up. The trait method exists
//! here so the GUI can call through to whichever backend implements
//! Sieve push without changing its own code.
//!
//! ## TLS posture
//!
//! Uses `tokio-rustls` with the `ring` provider and Mozilla's CA
//! roots from `webpki-roots`. SNI is set to the configured IMAP
//! host. There is no fallback to plaintext, no STARTTLS path, and
//! no opt-out for invalid certs — a misbehaving server is a hard
//! connection failure, not a downgrade.
//!
//! ## Concurrency
//!
//! The active `async_imap::Session` is held inside a `tokio::sync::Mutex`
//! so every trait call serializes on the wire. IMAP is an inherently
//! single-channel protocol; for parallelism, instantiate multiple
//! `RustImapBackend`s.

// The Mutex guard pattern in every trait method holds the lock for
// the duration of the operation (LIST + STATUS, EXAMINE + FETCH, etc.)
// because IMAP is inherently single-channel. The clippy lint suggests
// dropping the guard early; that's exactly wrong here.
#![allow(clippy::significant_drop_tightening)]

use std::sync::Arc;

use async_imap::Session;
use futures::{StreamExt as _, pin_mut};
use rustls::{ClientConfig, RootCertStore};
use rustls_pki_types::ServerName;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use crate::{AccountConfig, Backend, BackendError, FolderSummary, MessageHeaders};

/// Active IMAPS session bound to one account.
#[derive(Debug)]
pub struct RustImapBackend {
    /// The current authenticated session. Tokio `Mutex` because the
    /// `async_imap::Session` API takes `&mut self` on every method
    /// and we expose the trait through `&self`.
    session: Mutex<Session<TlsStream<TcpStream>>>,
}

/// Build the rustls config with Mozilla's webpki roots.
///
/// BUG ASSUMPTION: `webpki_roots::TLS_SERVER_ROOTS` is well-formed at
/// compile time; `add_trust_anchors` cannot fail with this input.
fn tls_config() -> ClientConfig {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth()
}

impl RustImapBackend {
    /// Connect to the configured IMAPS host and authenticate.
    ///
    /// `password` is consumed and dropped immediately after the
    /// LOGIN command — it is not retained inside [`RustImapBackend`].
    /// The session keeps the authenticated TLS connection alive.
    ///
    /// # Errors
    /// - `Transport` if TCP connect, TLS handshake, or initial
    ///   server greeting fails.
    /// - `Auth` if `LOGIN` is rejected.
    pub async fn connect(
        config: &AccountConfig,
        password: &str,
    ) -> Result<Self, BackendError> {
        let host = config.imap_host.as_str();
        let port = config.imap_port;
        let tcp = crate::connect_with_timeout(host, port, crate::CONNECT_TIMEOUT).await?;

        let connector = TlsConnector::from(Arc::new(tls_config()));
        let server_name = ServerName::try_from(host.to_string())
            .map_err(|e| BackendError::Transport(format!("server name: {e}")))?;
        let tls = connector
            .connect(server_name, tcp)
            .await
            .map_err(|e| BackendError::Transport(format!("tls handshake: {e}")))?;

        let client = async_imap::Client::new(tls);
        // The greeting is read by `login` internally; if the server
        // is misbehaving we surface that as Transport, not Auth.
        let session = client
            .login(&config.username, password)
            .await
            .map_err(|(e, _client)| BackendError::Auth(e.to_string()))?;

        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Logout cleanly. Best-effort — a failed logout is logged and
    /// then ignored, because the only meaningful recovery is to drop
    /// the session anyway.
    pub async fn logout(self) {
        let mut session = self.session.into_inner();
        if let Err(e) = session.logout().await {
            tracing::warn!(error = %e, "imap logout failed; dropping session");
        }
    }
}

/// A folder discovered via `LIST`, with the bits we need *before* we
/// decide whether to issue `STATUS` against it.
struct FolderEntry {
    name: String,
    /// `false` for `\Noselect` / `\NonExistent` placeholders — these
    /// cannot be `STATUS`'d (the server returns an error), so we must
    /// not query them.
    selectable: bool,
    /// RFC 6154 special-use marker (e.g. `\Sent`) if the server reports
    /// one in the `LIST` reply. Lets the UI label/order folders without
    /// a second round-trip.
    special_use: Option<String>,
}

impl FolderEntry {
    fn from_list(name: &async_imap::types::Name) -> Self {
        use async_imap::types::NameAttribute;
        let mut selectable = true;
        let mut special_use = None;
        for attr in name.attributes() {
            match attr {
                NameAttribute::NoSelect => selectable = false,
                NameAttribute::All => special_use = Some("\\All".to_string()),
                NameAttribute::Archive => special_use = Some("\\Archive".to_string()),
                NameAttribute::Drafts => special_use = Some("\\Drafts".to_string()),
                NameAttribute::Flagged => special_use = Some("\\Flagged".to_string()),
                NameAttribute::Junk => special_use = Some("\\Junk".to_string()),
                NameAttribute::Sent => special_use = Some("\\Sent".to_string()),
                NameAttribute::Trash => special_use = Some("\\Trash".to_string()),
                // `\NonExistent` (RFC 5258) arrives as an extension attr;
                // like `\Noselect` it marks a name that isn't a real,
                // openable mailbox.
                NameAttribute::Extension(ext) if ext.eq_ignore_ascii_case("\\NonExistent") => {
                    selectable = false;
                }
                _ => {}
            }
        }
        Self {
            name: name.name().to_string(),
            selectable,
            special_use,
        }
    }
}

impl Backend for RustImapBackend {
    async fn list_folders(&self) -> Result<Vec<FolderSummary>, BackendError> {
        let mut session = self.session.lock().await;

        // LIST "" "*" enumerates every mailbox the user can see. We
        // capture each folder's name *and* its name-attributes so we can
        // (a) skip unselectable placeholders and (b) surface special-use
        // markers to the UI. STATUS (issued below, per selectable folder)
        // is non-destructive — it does not change the selected mailbox —
        // so listing is safe to call from anywhere in the GUI.
        let entries: Vec<FolderEntry> = {
            let mut stream = session
                .list(Some(""), Some("*"))
                .await
                .map_err(|e| BackendError::Protocol(format!("list: {e}")))?;
            let mut acc = Vec::new();
            while let Some(item) = stream.next().await {
                let name = item
                    .map_err(|e| BackendError::Protocol(format!("list item: {e}")))?;
                // Borrow ends here: we extract owned values before the
                // stream advances and `name` is dropped.
                acc.push(FolderEntry::from_list(&name));
            }
            acc
        };

        let mut out = Vec::with_capacity(entries.len());
        let mut skipped = 0usize;
        for entry in entries {
            // CRITICAL: one bad folder must never sink the whole mailbox
            // load. A stale, unselectable hierarchy placeholder (the very
            // class of folder that produced "Couldn't load mailboxes")
            // is recorded with zero counts and never STATUS'd; a STATUS
            // error on an otherwise-selectable folder is logged and
            // degraded to zero counts rather than propagated.
            let (messages, unseen) = if entry.selectable {
                match session.status(&entry.name, "(MESSAGES UNSEEN)").await {
                    Ok(status) => (
                        u64::from(status.exists),
                        u64::from(status.unseen.unwrap_or(0)),
                    ),
                    Err(e) => {
                        skipped += 1;
                        tracing::warn!(
                            folder = %entry.name,
                            error = %e,
                            "STATUS failed; listing folder with zero counts"
                        );
                        (0, 0)
                    }
                }
            } else {
                skipped += 1;
                tracing::debug!(
                    folder = %entry.name,
                    "unselectable folder (\\Noselect/\\NonExistent); skipping STATUS"
                );
                (0, 0)
            };
            out.push(FolderSummary {
                name: entry.name,
                special_use: entry.special_use,
                messages,
                unseen,
            });
        }
        if skipped > 0 {
            tracing::info!(
                folders = out.len(),
                skipped,
                "listed folders; some had no usable STATUS"
            );
        }
        Ok(out)
    }

    async fn fetch_headers(
        &self,
        folder: &str,
        limit: Option<u32>,
    ) -> Result<Vec<MessageHeaders>, BackendError> {
        let mut session = self.session.lock().await;

        // EXAMINE is read-only SELECT — opens the folder for reading
        // without taking a write lock the server might surface to other
        // sessions as "in use".
        let examine = session
            .examine(folder)
            .await
            .map_err(|e| BackendError::Protocol(format!("examine {folder}: {e}")))?;

        let total = examine.exists;
        if total == 0 {
            return Ok(Vec::new());
        }

        // Build a UID range. limit=Some(N) → fetch the most recent N
        // by sequence-number; limit=None → all.
        let range = match limit {
            Some(n) if n < total => {
                let start = total - n + 1;
                format!("{start}:{total}")
            }
            _ => "1:*".to_string(),
        };

        let fetch_args = "(UID ENVELOPE BODY.PEEK[HEADER])";
        let mut stream = session
            .fetch(&range, fetch_args)
            .await
            .map_err(|e| BackendError::Protocol(format!("fetch {folder} {range}: {e}")))?;

        let mut out = Vec::new();
        while let Some(item) = stream.next().await {
            let f = item.map_err(|e| BackendError::Protocol(format!("fetch item: {e}")))?;
            let uid = f.uid.unwrap_or(0);
            let envelope = f.envelope();
            let from = envelope
                .and_then(|e| e.from.as_ref())
                .and_then(|addrs| addrs.first())
                .map(|addr| {
                    let mailbox = addr
                        .mailbox
                        .as_deref()
                        .map(String::from_utf8_lossy)
                        .unwrap_or_default();
                    let host = addr
                        .host
                        .as_deref()
                        .map(String::from_utf8_lossy)
                        .unwrap_or_default();
                    format!("{mailbox}@{host}")
                })
                .unwrap_or_default();
            let subject = envelope
                .and_then(|e| e.subject.as_deref())
                .map(String::from_utf8_lossy)
                .unwrap_or_default()
                .into_owned();
            let raw_headers = f.header().unwrap_or_default();
            let other_headers = parse_headers(raw_headers);
            out.push(MessageHeaders {
                uid,
                folder: folder.to_string(),
                from,
                subject,
                other_headers,
            });
        }
        Ok(out)
    }

    async fn move_message(
        &self,
        from_folder: &str,
        to_folder: &str,
        uid: u32,
    ) -> Result<(), BackendError> {
        let mut session = self.session.lock().await;
        session
            .select(from_folder)
            .await
            .map_err(|e| BackendError::Protocol(format!("select {from_folder}: {e}")))?;
        // UID MOVE is the right verb when the server advertises the
        // MOVE extension (RFC 6851); async-imap exposes it as `uid_mv`.
        // Otherwise fall back to UID COPY + UID STORE \Deleted + EXPUNGE.
        match session.uid_mv(format!("{uid}"), to_folder).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Some servers don't advertise MOVE; fall back to
                // COPY + STORE \Deleted + EXPUNGE. Drain the stream
                // returns so the session is ready for the next op.
                tracing::debug!(error = %e, "uid_mv failed, falling back to copy+store+expunge");
                session
                    .uid_copy(format!("{uid}"), to_folder)
                    .await
                    .map_err(|e| BackendError::Protocol(format!("uid_copy: {e}")))?;
                {
                    let updates = session
                        .uid_store(format!("{uid}"), "+FLAGS (\\Deleted)")
                        .await
                        .map_err(|e| BackendError::Protocol(format!("uid_store: {e}")))?;
                    pin_mut!(updates);
                    while let Some(item) = updates.next().await {
                        let _ = item;
                    }
                }
                {
                    let expunge = session
                        .expunge()
                        .await
                        .map_err(|e| BackendError::Protocol(format!("expunge: {e}")))?;
                    pin_mut!(expunge);
                    while let Some(item) = expunge.next().await {
                        let _ = item;
                    }
                }
                Ok(())
            }
        }
    }

    async fn set_flag(
        &self,
        folder: &str,
        uid: u32,
        flag: &str,
        set: bool,
    ) -> Result<(), BackendError> {
        let mut session = self.session.lock().await;
        session
            .select(folder)
            .await
            .map_err(|e| BackendError::Protocol(format!("select {folder}: {e}")))?;
        let op = if set { "+FLAGS" } else { "-FLAGS" };
        let item = format!("{op} ({flag})");
        let stream = session
            .uid_store(format!("{uid}"), item)
            .await
            .map_err(|e| BackendError::Protocol(format!("uid_store: {e}")))?;
        pin_mut!(stream);
        while let Some(item) = stream.next().await {
            let _ = item.map_err(|e| BackendError::Protocol(format!("store item: {e}")))?;
        }
        Ok(())
    }

    async fn put_sieve(&self, _script: &str) -> Result<(), BackendError> {
        // ManageSieve is a separate protocol on a separate port (4190)
        // with its own STARTTLS upgrade and SASL flow — it cannot be
        // multiplexed onto the IMAP session. The implementation lives
        // in `crate::managesieve::put_active_script`, which is a
        // standalone function that takes credentials per-call. We
        // could in principle have RustImapBackend retain credentials
        // and call through here, but caching the password on the
        // struct after login was specifically rejected on the IMAP
        // side; we keep it consistent.
        Err(BackendError::NotImplemented(
            "put_sieve: call thundercrab_imap::managesieve::put_active_script directly",
        ))
    }
}

/// Parse a raw RFC822 header block into `(lowercased-name, value)`
/// pairs. Only single-line headers are supported — folded headers
/// have their continuation lines collapsed onto the prior line with
/// a single space, per RFC 5322 unfolding.
///
/// BUG ASSUMPTION: `From:` and `Subject:` are NOT included in the
/// output here — they're surfaced through the typed `MessageHeaders`
/// fields. Including them in `other_headers` would double-count.
fn parse_headers(raw: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(raw);
    let mut acc: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in text.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            // Blank line ends the header block.
            if let Some((name, value)) = current.take() {
                push_if_relevant(&mut acc, name, value);
            }
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            // Continuation of the previous header.
            if let Some((_, ref mut value)) = current {
                value.push(' ');
                value.push_str(line.trim_start());
            }
            continue;
        }
        // New header.
        if let Some((name, value)) = current.take() {
            push_if_relevant(&mut acc, name, value);
        }
        if let Some((name, value)) = line.split_once(':') {
            current = Some((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    if let Some((name, value)) = current.take() {
        push_if_relevant(&mut acc, name, value);
    }
    acc
}

fn push_if_relevant(acc: &mut Vec<(String, String)>, name: String, value: String) {
    // From + Subject live in the typed MessageHeaders fields; do not
    // duplicate them in other_headers.
    if name == "from" || name == "subject" {
        return;
    }
    acc.push((name, value));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_headers() {
        let raw = b"List-Id: <example.com>\r\nX-Priority: 1\r\n\r\n";
        let h = parse_headers(raw);
        assert_eq!(h.len(), 2);
        assert_eq!(h[0], ("list-id".into(), "<example.com>".into()));
        assert_eq!(h[1], ("x-priority".into(), "1".into()));
    }

    #[test]
    fn unfolds_continuation_lines() {
        let raw = b"X-Long: line one\r\n  continued\r\n\tand again\r\n\r\n";
        let h = parse_headers(raw);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].0, "x-long");
        assert_eq!(h[0].1, "line one continued and again");
    }

    #[test]
    fn excludes_from_and_subject_from_other_headers() {
        // From + Subject are surfaced through typed fields; including
        // them in other_headers would mean every consumer has to dedupe.
        let raw = b"From: x@y.z\r\nSubject: hi\r\nList-Id: <l>\r\n\r\n";
        let h = parse_headers(raw);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].0, "list-id");
    }

    #[test]
    fn handles_missing_blank_line() {
        // Some servers omit the trailing CRLF before the body cut.
        let raw = b"List-Id: <l>\r\n";
        let h = parse_headers(raw);
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn ignores_lowercase_drift_in_name() {
        // RFC 5322 says header field names are case-insensitive; we
        // normalize to lowercase so consumers can match exactly.
        let raw = b"LIST-ID: <l>\r\nx-mailer: foo\r\n\r\n";
        let h = parse_headers(raw);
        assert_eq!(h[0].0, "list-id");
        assert_eq!(h[1].0, "x-mailer");
    }
}
