//! `thundercrab-imap` — `IMAP4rev2` / SMTP / `ManageSieve` client layer.
//!
//! Provides the typed [`AccountConfig`], [`Backend`] trait, and a
//! concrete IMAPS backend ([`rust_imap::RustImapBackend`]) using
//! `async-imap` over `tokio-rustls`. SMTP and `ManageSieve` sit on
//! the same trait surface — separate-port protocols implemented in
//! sibling modules; tracked as thundercrab #60 / #61.
//!
//! Why a trait at all: GUI / suggestion code depends on `Backend`,
//! not on the concrete IMAP wire. Swapping in a `MockBackend` for
//! tests, a `JmapBackend` for JMAP servers, or a `LocalMaildirBackend`
//! for offline-only mode is then a matter of satisfying the trait
//! — not editing UI code.

#![doc(html_no_source)]

pub mod managesieve;
pub mod rust_imap;
pub mod smtp;

use std::future::Future;
use thiserror::Error;

/// Errors returned by a [`Backend`].
#[derive(Debug, Error)]
pub enum BackendError {
    /// Network or transport failure.
    #[error("transport: {0}")]
    Transport(String),
    /// Authentication or authorization failure.
    #[error("auth: {0}")]
    Auth(String),
    /// Server returned a protocol-level error response.
    #[error("protocol: {0}")]
    Protocol(String),
    /// Feature requested is not implemented in this backend yet.
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

/// Server connection parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountConfig {
    /// Hostname for IMAP and `ManageSieve` (assumed colocated; SMTP may
    /// differ, see `smtp_host`).
    pub imap_host: String,
    /// IMAPS port (default 993).
    pub imap_port: u16,
    /// SMTP submission host (default same as `imap_host`).
    pub smtp_host: String,
    /// SMTP submission port (default 587 STARTTLS, 465 implicit TLS).
    pub smtp_port: u16,
    /// `ManageSieve` port (default 4190).
    pub sieve_port: u16,
    /// Username (full email address).
    pub username: String,
}

impl AccountConfig {
    /// PlausiDen-style defaults: same host for IMAP/SMTP/ManageSieve,
    /// standard ports.
    #[must_use]
    pub fn plausiden(host: impl Into<String>, username: impl Into<String>) -> Self {
        let host = host.into();
        Self {
            imap_host: host.clone(),
            imap_port: 993,
            smtp_host: host,
            smtp_port: 587,
            sieve_port: 4190,
            username: username.into(),
        }
    }
}

/// A folder summary as returned by [`Backend::list_folders`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderSummary {
    /// Folder name as the server presents it.
    pub name: String,
    /// IMAP special-use marker (`\Sent`, `\Drafts`, etc.) if any.
    pub special_use: Option<String>,
    /// Total message count.
    pub messages: u64,
    /// Unread count.
    pub unseen: u64,
}

/// A message header summary — the data the rules engine needs.
#[derive(Debug, Clone)]
pub struct MessageHeaders {
    /// IMAP UID.
    pub uid: u32,
    /// Folder this message lives in (server-side path).
    pub folder: String,
    /// `From:` header value (raw, single-line).
    pub from: String,
    /// `Subject:` header value (raw, single-line).
    pub subject: String,
    /// `(name, value)` pairs of additional headers we capture for rule
    /// evaluation: `List-Id`, `List-Unsubscribe`, `X-Priority`, etc.
    /// Names lowercased; values original-case.
    pub other_headers: Vec<(String, String)>,
}

/// Backend-agnostic mailbox operations.
///
/// BUG ASSUMPTION: All methods are async and assume the underlying
/// connection is single-threaded. Callers wanting concurrency open
/// multiple connections.
pub trait Backend: Send + Sync {
    /// List all folders the user can see.
    fn list_folders(&self)
    -> impl Future<Output = Result<Vec<FolderSummary>, BackendError>> + Send;

    /// Fetch headers for messages in a folder, optionally limited.
    /// `limit = None` means "everything".
    fn fetch_headers(
        &self,
        folder: &str,
        limit: Option<u32>,
    ) -> impl Future<Output = Result<Vec<MessageHeaders>, BackendError>> + Send;

    /// Move `uid` from `from_folder` to `to_folder`.
    fn move_message(
        &self,
        from_folder: &str,
        to_folder: &str,
        uid: u32,
    ) -> impl Future<Output = Result<(), BackendError>> + Send;

    /// Set or clear an IMAP flag on `uid` in `folder`.
    fn set_flag(
        &self,
        folder: &str,
        uid: u32,
        flag: &str,
        set: bool,
    ) -> impl Future<Output = Result<(), BackendError>> + Send;

    /// Push the active server-side Sieve script via `ManageSieve`.
    /// Empty `script` is a valid "clear it" call.
    fn put_sieve(&self, script: &str) -> impl Future<Output = Result<(), BackendError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plausiden_defaults_use_standard_ports() {
        let cfg = AccountConfig::plausiden("mail.plausiden.com", "team@plausiden.com");
        assert_eq!(cfg.imap_port, 993);
        assert_eq!(cfg.smtp_port, 587);
        assert_eq!(cfg.sieve_port, 4190);
        assert_eq!(cfg.imap_host, "mail.plausiden.com");
        assert_eq!(cfg.smtp_host, "mail.plausiden.com");
    }
}
