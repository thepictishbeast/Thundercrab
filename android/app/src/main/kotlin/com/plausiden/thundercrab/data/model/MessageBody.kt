// ============================================================================
// data/model/MessageBody.kt  (DATA group)
// A received message's display body. DATA-owned domain model — NO Ffi* type.
// Mapped from uniffi.thundercrab_ffi.FfiMessageBody inside the Repository.
//
// The HTML is ALREADY SANITIZED by the Rust core (scripts, event handlers, and
// all remote content removed) — safe to render directly. Body reading is a
// display-only carve-out: it never feeds the rules / suggestions / ledger spine.
// ============================================================================
package com.plausiden.thundercrab.data.model

/**
 * A received message body, ready to display.
 *
 * @param plain          the text/plain part, if any.
 * @param htmlSanitized  the HTML representation, already sanitized by the core
 *                       (safe to load in a locked-down WebView). Null when there
 *                       is no renderable body.
 */
data class MessageBody(
    val plain: String?,
    val htmlSanitized: String?,
)
