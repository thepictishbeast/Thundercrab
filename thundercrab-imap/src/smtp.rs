//! SMTP submission client (RFC 5321 + RFC 6409) — outbound mail.
//!
//! ThunderCrab uses lettre for the wire-level submission protocol;
//! this module is the thin wrapper that translates a ThunderCrab
//! `OutboundMessage` into a `lettre::Message` and ships it through
//! a `tokio1-rustls-tls` SMTP transport. The credentials and host
//! come from the same `AccountConfig` the IMAP and `ManageSieve`
//! modules use, so a single account is wired end-to-end.
//!
//! ## TLS posture
//!
//! Submission on port 587 with STARTTLS by default (RFC 6409).
//! Port 465 with implicit TLS is also supported via
//! [`SmtpEncryption::ImplicitTls`]. Both paths use rustls under the
//! hood; lettre's `tokio1-rustls-tls` feature inherits Mozilla's
//! webpki roots, so the trust posture matches our IMAP and
//! `ManageSieve` modules.
//!
//! There is no plaintext-submission fallback. A misbehaving server
//! is a hard failure, not a downgrade.
//!
//! ## What this isn't
//!
//! * Not a queue. If the network is down, the call returns
//!   `Transport`. Retry / queue is a higher-layer concern.
//! * Not a relay. Authenticates as the account user; the user is
//!   the From: principal. Sending as someone else is out of scope.
//! * Not a bulk sender. Per-message connection (lettre handles
//!   pooling internally, but ThunderCrab's GUI submits one at a
//!   time anyway).

use lettre::message::Mailbox;
use lettre::transport::smtp::AsyncSmtpTransport;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncTransport, Message, Tokio1Executor};

use crate::{AccountConfig, BackendError};

/// TLS strategy for the SMTP submission connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpEncryption {
    /// STARTTLS upgrade on the submission port (default 587).
    StartTls,
    /// Implicit TLS from byte 0 (default port 465).
    ImplicitTls,
}

/// ThunderCrab-side message description. Translated to a
/// `lettre::Message` inside [`send_message`]; lettre then handles
/// MIME encoding, address validation, and the wire submission.
#[derive(Debug, Clone)]
pub struct OutboundMessage<'a> {
    /// `From:` mailbox. Must match the authenticated account on
    /// most submission servers (`PlausiDen` mail enforces this).
    pub from: &'a str,
    /// Recipients on the To: line.
    pub to: &'a [&'a str],
    /// Recipients on the Cc: line.
    pub cc: &'a [&'a str],
    /// `Subject:` line.
    pub subject: &'a str,
    /// Plain-text body. ThunderCrab's v0 sends text/plain only;
    /// HTML bodies are a follow-up once the editor exists.
    pub body: &'a str,
}

/// Connect, authenticate, send one message, close the transport.
///
/// # Errors
/// - `Transport` if any network / TLS / SMTP-protocol error occurs.
/// - `Auth` if the SMTP AUTH step is rejected.
pub async fn send_message(
    cfg: &AccountConfig,
    password: &str,
    encryption: SmtpEncryption,
    message: &OutboundMessage<'_>,
) -> Result<(), BackendError> {
    // lettre builds its own rustls ClientConfig internally via the no-provider
    // `builder()`. Since both `ring` (lettre) and `aws-lc-rs` (us) are compiled
    // in, that call needs a process-default provider installed first — and this
    // also gives the SMTP path the same PQ-capable provider as IMAP/Sieve.
    crate::tls::ensure_provider();

    let mut builder = Message::builder()
        .from(parse_mailbox(message.from)?)
        .subject(message.subject);
    for to in message.to {
        builder = builder.to(parse_mailbox(to)?);
    }
    for cc in message.cc {
        builder = builder.cc(parse_mailbox(cc)?);
    }
    let email = builder
        .body(message.body.to_string())
        .map_err(|e| BackendError::Protocol(format!("compose: {e}")))?;

    let creds = Credentials::new(cfg.username.clone(), password.to_string());
    let transport = match encryption {
        SmtpEncryption::StartTls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.smtp_host)
                .map_err(|e| BackendError::Transport(format!("smtp builder: {e}")))?
                .port(cfg.smtp_port)
                .credentials(creds)
                .timeout(Some(crate::CONNECT_TIMEOUT)) // explicit + consistent w/ IMAP/sieve (lettre default is 60s)
                .build()
        }
        SmtpEncryption::ImplicitTls => {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.smtp_host)
                .map_err(|e| BackendError::Transport(format!("smtp builder: {e}")))?
                .port(cfg.smtp_port)
                .credentials(creds)
                .timeout(Some(crate::CONNECT_TIMEOUT)) // explicit + consistent w/ IMAP/sieve (lettre default is 60s)
                .build()
        }
    };

    transport
        .send(email)
        .await
        .map(|_response| ())
        .map_err(|e| BackendError::Transport(format!("smtp send: {e}")))
}

fn parse_mailbox(s: &str) -> Result<Mailbox, BackendError> {
    s.parse::<Mailbox>()
        .map_err(|e| BackendError::Protocol(format!("mailbox {s:?}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mailbox_accepts_plain_address() {
        let m = parse_mailbox("user@example.com").expect("plain address parses");
        assert_eq!(m.email.to_string(), "user@example.com");
    }

    #[test]
    fn parse_mailbox_accepts_display_form() {
        let m = parse_mailbox("Display Name <user@example.com>")
            .expect("display-form parses");
        assert_eq!(m.email.to_string(), "user@example.com");
        assert_eq!(m.name.as_deref(), Some("Display Name"));
    }

    #[test]
    fn parse_mailbox_rejects_garbage() {
        assert!(parse_mailbox("not an address").is_err());
        assert!(parse_mailbox("").is_err());
    }
}
