//! Inbound message-body extraction.
//!
//! [`fetch_body`](crate::rust_imap::RustImapBackend::fetch_body) pulls a full
//! message off the server; [`parse_body`] turns the raw RFC 5322 / MIME bytes
//! into a display-ready [`MessageBody`]. The HTML part is **sanitized here**
//! (via [`thundercrab_core::mail_html::sanitize_html`]) before it leaves this
//! module, so no consumer can ever render unsafe or network-reaching HTML.

use mail_parser::MimeHeaders;
use thundercrab_core::mail_html::sanitize_html;

/// One attachment carried by a message: metadata plus the decoded bytes.
/// Bytes are content-transfer-decoded by `mail-parser`, so they are the real
/// file contents ready to write to disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attachment {
    /// The declared filename (from Content-Disposition / Content-Type `name`).
    /// Empty when the part is unnamed — consumers should synthesize one.
    pub filename: String,
    /// MIME type as `type/subtype`, lowercased (e.g. `image/png`). Falls back
    /// to `application/octet-stream` when the part declares no content type.
    pub mime_type: String,
    /// The decoded attachment bytes.
    pub bytes: Vec<u8>,
}

impl Attachment {
    /// Size of the decoded attachment in bytes.
    #[must_use]
    pub fn size(&self) -> usize {
        self.bytes.len()
    }
}

/// The display body of a received message. Either text part may be absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageBody {
    /// `text/plain` part, decoded, as-is. `None` if the message has no text part.
    pub plain: Option<String>,
    /// The message's HTML representation, **already sanitized** (scripts, event
    /// handlers, and all remote content removed) — safe to hand straight to a
    /// renderer. This is a real `text/html` part when one exists, otherwise the
    /// text body converted to HTML (mail-parser's fallback). `None` only when
    /// the message has no renderable body at all.
    pub html_sanitized: Option<String>,
    /// Attachments carried by the message, in document order. Empty when none.
    /// The body's own text/html parts are NOT included here.
    pub attachments: Vec<Attachment>,
}

/// Parse raw MIME bytes into a [`MessageBody`], extracting the primary
/// `text/plain` and `text/html` parts. The HTML is run through
/// [`sanitize_html`] before return.
///
/// Lenient: a message that fails to parse yields an empty [`MessageBody`]
/// rather than an error — a malformed message should degrade to "no body
/// shown," never sink the mailbox.
#[must_use]
pub fn parse_body(raw: &[u8]) -> MessageBody {
    let Some(message) = mail_parser::MessageParser::default().parse(raw) else {
        return MessageBody::default();
    };
    // mail-parser separates body parts from attachments, so iterating
    // `attachments()` never double-counts the text/html body we extract above.
    let attachments = message
        .attachments()
        .map(|part| {
            let mime_type = part.content_type().map_or_else(
                || "application/octet-stream".to_string(),
                |ct| match ct.subtype() {
                    Some(sub) => format!("{}/{}", ct.ctype(), sub).to_lowercase(),
                    None => ct.ctype().to_lowercase(),
                },
            );
            Attachment {
                filename: part.attachment_name().unwrap_or_default().to_string(),
                mime_type,
                bytes: part.contents().to_vec(),
            }
        })
        .collect();

    // `body_html` returns a real text/html part when present, else the text
    // body converted to HTML. Either way it is sanitized before return, so any
    // renderer receives safe, network-free HTML.
    MessageBody {
        plain: message.body_text(0).map(|c| c.into_owned()),
        html_sanitized: message.body_html(0).map(|c| sanitize_html(&c)),
        attachments,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_body;

    // A multipart/alternative message: plain fallback + an HTML part that
    // carries a script and a remote tracking image.
    const MULTIPART: &[u8] = b"From: a@b.c\r\nTo: d@e.f\r\nSubject: hi\r\n\
MIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"X\"\r\n\r\n\
--X\r\nContent-Type: text/plain\r\n\r\nplain body here\r\n\
--X\r\nContent-Type: text/html\r\n\r\n\
<p>rich body</p><script>evil()</script><img src=\"https://t.example/p.gif\">\r\n\
--X--\r\n";

    #[test]
    fn extracts_plain_and_sanitized_html() {
        let b = parse_body(MULTIPART);
        assert!(
            b.plain.as_deref().unwrap_or_default().contains("plain body here"),
            "plain extracted: {:?}",
            b.plain
        );
        let html = b.html_sanitized.expect("html part present");
        assert!(html.contains("rich body"), "html content kept: {html}");
        assert!(!html.to_lowercase().contains("<script"), "script stripped: {html}");
        assert!(!html.contains("t.example"), "remote tracking img stripped: {html}");
    }

    #[test]
    fn plain_only_message_exposes_plain_and_safe_html() {
        let raw = b"From: a@b.c\r\nSubject: x\r\nContent-Type: text/plain\r\n\r\njust text";
        let b = parse_body(raw);
        assert!(b.plain.as_deref().unwrap_or_default().contains("just text"));
        // mail-parser synthesizes an HTML view from the text when there's no
        // real HTML part; whatever it returns must still be safe (sanitized).
        if let Some(html) = b.html_sanitized {
            assert!(html.contains("just text"), "synthesized html carries the text: {html}");
            assert!(!html.to_lowercase().contains("<script"), "still sanitized: {html}");
        }
    }

    // multipart/mixed: a text body plus a small named binary attachment.
    const WITH_ATTACHMENT: &[u8] = b"From: a@b.c\r\nTo: d@e.f\r\nSubject: doc\r\n\
MIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"M\"\r\n\r\n\
--M\r\nContent-Type: text/plain\r\n\r\nsee attached\r\n\
--M\r\nContent-Type: application/pdf; name=\"report.pdf\"\r\n\
Content-Disposition: attachment; filename=\"report.pdf\"\r\n\
Content-Transfer-Encoding: base64\r\n\r\nSGVsbG8gUERG\r\n\
--M--\r\n";

    #[test]
    fn extracts_attachment_metadata_and_decoded_bytes() {
        let b = parse_body(WITH_ATTACHMENT);
        assert!(b.plain.as_deref().unwrap_or_default().contains("see attached"));
        assert_eq!(b.attachments.len(), 1, "one attachment parsed");
        let att = &b.attachments[0];
        assert_eq!(att.filename, "report.pdf");
        assert_eq!(att.mime_type, "application/pdf");
        // base64 "SGVsbG8gUERG" decodes to "Hello PDF" — content-transfer-decoded.
        assert_eq!(att.bytes, b"Hello PDF");
        assert_eq!(att.size(), 9);
    }

    #[test]
    fn body_without_attachments_has_empty_vec() {
        let b = parse_body(MULTIPART);
        assert!(b.attachments.is_empty(), "alternative parts are not attachments");
    }

    #[test]
    fn unparseable_input_degrades_to_empty() {
        // Must not panic; both parts simply absent / harmless.
        let b = parse_body(b"this is not a real email at all");
        assert!(b.html_sanitized.is_none());
    }
}
