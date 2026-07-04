//! Pure reply/forward derivations (RFC 5322 §3.6.4).
//!
//! Subject prefixing, quoting, the forwarded block, the `References` chain, and
//! a case-insensitive header lookup. No IO — deterministic string functions,
//! unit-tested. The Android client mirrors this behavior in its own Kotlin
//! `ReplyForward`; this Rust copy serves the desktop GUI and CLI.

use std::fmt::Write as _;

/// `Re: <subject>`, unless it already starts with a `Re:` prefix (any case).
#[must_use]
pub fn reply_subject(original: &str) -> String {
    let s = original.trim();
    if starts_with_prefix(s, &["re"]) {
        s.to_string()
    } else {
        format!("Re: {s}")
    }
}

/// `Fwd: <subject>`, unless it already starts with an `Fwd:`/`Fw:` prefix.
#[must_use]
pub fn forward_subject(original: &str) -> String {
    let s = original.trim();
    if starts_with_prefix(s, &["fwd", "fw"]) {
        s.to_string()
    } else {
        format!("Fwd: {s}")
    }
}

/// Case-insensitive: does `s` start with any `prefix` then optional whitespace
/// then a colon (e.g. `Re:`, `RE :`, `Fwd:`)? Guards against false hits like
/// `Fwiw:` (which is not a forward prefix).
fn starts_with_prefix(s: &str, prefixes: &[&str]) -> bool {
    let lower = s.to_ascii_lowercase();
    prefixes.iter().any(|p| {
        lower
            .strip_prefix(p)
            .is_some_and(|rest| rest.trim_start().starts_with(':'))
    })
}

/// Top-posted reply body: two blank lines for the user to type, then an
/// attribution line and the original quoted with `> ` (a Markdown blockquote).
#[must_use]
pub fn quoted_reply(from: &str, date: &str, body: &str) -> String {
    let quoted = body
        .trim_end_matches('\n')
        .lines()
        .map(|l| if l.is_empty() { ">".to_string() } else { format!("> {l}") })
        .collect::<Vec<_>>()
        .join("\n");
    let attribution = if date.trim().is_empty() {
        format!("On {from} wrote:")
    } else {
        format!("On {date}, {from} wrote:")
    };
    format!("\n\n{attribution}\n{quoted}\n")
}

/// Forwarded-message block: two blank lines, a header banner, then the body.
#[must_use]
pub fn forwarded_body(from: &str, date: &str, subject: &str, to: &str, body: &str) -> String {
    let mut out = String::from("\n\n---------- Forwarded message ----------\n");
    // Writing to a String is infallible; the discarded Result satisfies clippy.
    let _ = writeln!(out, "From: {from}");
    if !date.trim().is_empty() {
        let _ = writeln!(out, "Date: {date}");
    }
    let _ = writeln!(out, "Subject: {subject}");
    if !to.trim().is_empty() {
        let _ = writeln!(out, "To: {to}");
    }
    out.push('\n');
    out.push_str(body.trim_end_matches('\n'));
    out.push('\n');
    out
}

/// The reply's `References` header: the original chain with the original
/// `Message-ID` appended; or just the Message-ID; or just the chain; or `None`.
#[must_use]
pub fn references_chain(
    original_references: Option<&str>,
    original_message_id: Option<&str>,
) -> Option<String> {
    let refs = original_references.map(str::trim).filter(|s| !s.is_empty());
    let mid = original_message_id.map(str::trim).filter(|s| !s.is_empty());
    match (refs, mid) {
        (Some(r), Some(m)) => Some(format!("{r} {m}")),
        (None, Some(m)) => Some(m.to_string()),
        (Some(r), None) => Some(r.to_string()),
        (None, None) => None,
    }
}

/// Case-insensitive lookup over a `(name, value)` header list. Names in
/// `MessageHeaders::other_headers` arrive lowercased, but comparing
/// case-insensitively keeps callers robust.
#[must_use]
pub fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_adds_re() {
        assert_eq!(reply_subject("Hello"), "Re: Hello");
    }

    #[test]
    fn reply_does_not_double_re() {
        assert_eq!(reply_subject("Re: Hello"), "Re: Hello");
        assert_eq!(reply_subject("RE: Hello"), "RE: Hello");
    }

    #[test]
    fn reply_to_forward_still_adds_re() {
        assert_eq!(reply_subject("Fwd: Hello"), "Re: Fwd: Hello");
    }

    #[test]
    fn forward_adds_fwd() {
        assert_eq!(forward_subject("Hello"), "Fwd: Hello");
    }

    #[test]
    fn forward_recognizes_fw_and_fwd() {
        assert_eq!(forward_subject("Fwd: Hello"), "Fwd: Hello");
        assert_eq!(forward_subject("Fw: Hello"), "Fw: Hello");
    }

    #[test]
    fn forward_not_fooled_by_fwiw() {
        assert_eq!(forward_subject("Fwiw: note"), "Fwd: Fwiw: note");
    }

    #[test]
    fn references_appends_message_id() {
        assert_eq!(
            references_chain(Some("<a@h> <b@h>"), Some("<c@h>")).as_deref(),
            Some("<a@h> <b@h> <c@h>"),
        );
    }

    #[test]
    fn references_fallbacks() {
        assert_eq!(references_chain(None, Some("<c@h>")).as_deref(), Some("<c@h>"));
        assert_eq!(references_chain(Some("<a@h>"), None).as_deref(), Some("<a@h>"));
        assert_eq!(references_chain(None, None), None);
    }

    #[test]
    fn quoted_reply_prefixes_lines() {
        let out = quoted_reply("a@h", "Wed, 2 Jul 2026", "line1\nline2");
        assert!(out.contains("On Wed, 2 Jul 2026, a@h wrote:"));
        assert!(out.contains("> line1"));
        assert!(out.contains("> line2"));
    }

    #[test]
    fn quoted_reply_omits_blank_date() {
        assert!(quoted_reply("a@h", "", "x").contains("On a@h wrote:"));
    }

    #[test]
    fn forwarded_body_has_banner() {
        let out = forwarded_body("a@h", "d", "subj", "b@h", "body");
        assert!(out.contains("---------- Forwarded message ----------"));
        assert!(out.contains("From: a@h"));
        assert!(out.contains("Subject: subj"));
        assert!(out.contains("body"));
    }

    #[test]
    fn header_value_case_insensitive() {
        let h = vec![("message-id".to_string(), "<x@h>".to_string())];
        assert_eq!(header_value(&h, "Message-ID"), Some("<x@h>"));
        assert_eq!(header_value(&h, "reply-to"), None);
    }
}
