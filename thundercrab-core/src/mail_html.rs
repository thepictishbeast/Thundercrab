//! Safe rendering of received HTML mail.
//!
//! Inbound HTML is hostile by default: it can carry scripts, event handlers,
//! `javascript:` URLs, and — most insidiously — remote resource references
//! (`<img src="https://tracker/pixel.gif">`) whose mere *loading* reports back
//! "this message was opened, from this IP, at this time."
//!
//! [`sanitize_html`] runs every received HTML body through [`ammonia`] (an
//! allowlist sanitizer) and a custom attribute filter that additionally
//! **blocks remote content**. The result is safe to hand to any renderer
//! (Android `WebView`, desktop HTML view) without it phoning home — the
//! privacy posture `FairEmail` takes by default.

use std::borrow::Cow;

/// Sanitize a received HTML body for safe display.
///
/// Guarantees:
/// - No scripts, `<style>`, event handlers (`onclick`, …), or `javascript:` /
///   other dangerous URL schemes survive — ammonia's allowlist removes them.
/// - **No content loads from the network.** Every resource-loading attribute
///   (`src` / `srcset` / `background` / `poster`) is dropped, so opening a
///   message can never fetch a tracking pixel or beacon. This is the
///   "block remote content by default" posture `FairEmail` ships with.
/// - Hyperlinks (`<a href>`) to safe schemes are preserved — the user chooses
///   whether to click them — and ammonia rewrites them with
///   `rel="noopener noreferrer"`.
///
/// FOLLOW-UP: inline images referenced by `cid:` (multipart/related) are
/// stripped here too. Re-enabling them safely requires resolving the `cid:`
/// reference to its sibling MIME part and rewriting it to a local `data:` URI
/// during body assembly — tracked with the inbound-render work, not bolted on
/// here where we cannot see the other parts.
#[must_use]
pub fn sanitize_html(raw: &str) -> String {
    ammonia::Builder::default()
        .attribute_filter(|_element, attribute, value| match attribute {
            // Drop every resource-loading attribute. We never want rendering
            // to reach the network; inline-image support is a deliberate
            // future opt-in (see FOLLOW-UP above), not an accident of which
            // URL schemes ammonia happens to allow.
            "src" | "srcset" | "background" | "poster" => None,
            _ => Some(Cow::Borrowed(value)),
        })
        .clean(raw)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::sanitize_html;

    #[test]
    fn strips_script_tags() {
        let out = sanitize_html("<p>hi</p><script>alert(1)</script>");
        assert!(out.contains("hi"));
        assert!(!out.to_lowercase().contains("<script"), "no script tag: {out}");
        assert!(!out.contains("alert(1)"), "no script body: {out}");
    }

    #[test]
    fn strips_event_handlers_and_js_urls() {
        let out = sanitize_html(r#"<a href="javascript:alert(1)" onclick="x()">click</a>"#);
        assert!(!out.to_lowercase().contains("javascript:"), "no js url: {out}");
        assert!(!out.to_lowercase().contains("onclick"), "no handler: {out}");
        assert!(out.contains("click"), "link text kept: {out}");
    }

    #[test]
    fn blocks_remote_image_tracking_pixels() {
        let out = sanitize_html(r#"<p>hi</p><img src="https://tracker.example/p.gif?id=123">"#);
        assert!(
            !out.contains("tracker.example"),
            "remote src must be stripped so render can't phone home: {out}"
        );
        assert!(!out.contains("https://"), "no remote url remains: {out}");
        assert!(out.contains("hi"), "surrounding content preserved: {out}");
    }

    #[test]
    fn blocks_all_image_loads_including_inline() {
        // Privacy default: no image source survives — remote (tracking) or
        // inline (cid:/data:). Inline rendering is a future opt-in that needs
        // MIME-part resolution; until then, nothing loads.
        for raw in [
            r#"<img src="cid:logo@thundercrab">"#,
            r#"<img src="data:image/png;base64,AAAA">"#,
            r#"<img src="https://tracker.example/p.gif">"#,
        ] {
            let out = sanitize_html(raw);
            assert!(!out.contains("src"), "no image src survives: {raw} -> {out}");
        }
    }

    #[test]
    fn keeps_safe_formatting_and_links() {
        let out = sanitize_html(
            "<p><b>bold</b> <i>italic</i> <a href=\"https://example.com\">link</a></p>",
        );
        assert!(out.contains("<b>bold</b>"), "bold kept: {out}");
        assert!(out.contains("<i>italic</i>"), "italic kept: {out}");
        // The link itself (a non-resource-loading reference) survives.
        assert!(out.contains("example.com"), "safe link kept: {out}");
    }
}
