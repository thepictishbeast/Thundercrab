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

use lettre::message::{Mailbox, MultiPart};
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
    /// Plain-text body. Always sent — it is the universal fallback part
    /// of `multipart/alternative` (and the whole message when `html_body`
    /// is `None`). For a ThunderCrab Markdown compose, this is the source
    /// Markdown, which stays perfectly readable in any plain-text client.
    pub body: &'a str,
    /// Optional HTML body. When `Some`, the message is sent as
    /// `multipart/alternative` (`text/plain` first, `text/html` second) so
    /// every client renders one part: ThunderCrab/HTML-capable clients show
    /// the HTML, plain-text clients fall back to [`OutboundMessage::body`].
    ///
    /// SECURITY: the caller is responsible for producing trustworthy HTML
    /// (e.g. rendering ThunderCrab Markdown through a sanitizing pipeline).
    /// This module does not sanitize — it only frames the parts.
    pub html_body: Option<&'a str>,
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

    let email = build_message(message)?;

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

/// Translate an [`OutboundMessage`] into a `lettre::Message`, framing the
/// body as `multipart/alternative` (`text/plain` + `text/html`) when an HTML
/// body is present, or a single `text/plain` part otherwise.
///
/// Pure (no I/O) so the MIME-shaping decision is unit-testable without a
/// network round-trip.
///
/// # Errors
/// `Protocol` if any address is malformed or lettre rejects the composed body.
fn build_message(message: &OutboundMessage<'_>) -> Result<Message, BackendError> {
    let mut builder = Message::builder()
        .from(parse_mailbox(message.from)?)
        .subject(message.subject);
    for to in message.to {
        builder = builder.to(parse_mailbox(to)?);
    }
    for cc in message.cc {
        builder = builder.cc(parse_mailbox(cc)?);
    }

    let composed = match message.html_body {
        // text/plain first, text/html second: per MIME, clients render the
        // LAST part they understand, so HTML-capable clients show the HTML
        // while plain-text clients fall back to the plain part.
        Some(html) => builder.multipart(MultiPart::alternative_plain_html(
            message.body.to_string(),
            html.to_string(),
        )),
        None => builder.body(message.body.to_string()),
    };
    composed.map_err(|e| BackendError::Protocol(format!("compose: {e}")))
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

    fn msg<'a>(body: &'a str, html: Option<&'a str>) -> OutboundMessage<'a> {
        OutboundMessage {
            from: "sender@example.com",
            to: &["rcpt@example.com"],
            cc: &[],
            subject: "hi",
            body,
            html_body: html,
        }
    }

    #[test]
    fn plain_only_message_is_single_part() {
        // lettre emits a bare single-part body (no explicit Content-Type
        // header) for `.body()`, so the meaningful invariant is: NOT
        // multipart, and the body is present verbatim.
        let email = build_message(&msg("just text", None)).expect("builds");
        let wire = String::from_utf8(email.formatted()).expect("utf8");
        assert!(
            !wire.contains("multipart/alternative"),
            "no multipart when html_body is None:\n{wire}"
        );
        assert!(!wire.contains("text/html"), "no html part:\n{wire}");
        assert!(wire.contains("just text"), "plain body present:\n{wire}");
    }

    #[test]
    fn html_message_is_multipart_alternative_with_both_parts() {
        let email =
            build_message(&msg("plain fallback", Some("<p>rich</p>"))).expect("builds");
        let wire = String::from_utf8(email.formatted()).expect("utf8");
        assert!(
            wire.contains("multipart/alternative"),
            "multipart/alternative when html present:\n{wire}"
        );
        // Both alternatives must ship so non-HTML clients have the fallback.
        assert!(wire.contains("text/plain"), "plain fallback part present");
        assert!(wire.contains("text/html"), "html part present");
    }

    #[test]
    fn build_message_rejects_bad_address() {
        let bad = OutboundMessage {
            from: "garbage",
            to: &["rcpt@example.com"],
            cc: &[],
            subject: "hi",
            body: "x",
            html_body: None,
        };
        assert!(build_message(&bad).is_err());
    }
}
