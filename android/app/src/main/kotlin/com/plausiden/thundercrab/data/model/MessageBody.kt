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
    /** Attachments carried by the message, in document order. Bytes are fetched
     *  on demand (see [com.plausiden.thundercrab.data.ThunderCrabRepository.fetchAttachment]). */
    val attachments: List<Attachment> = emptyList(),
)

/**
 * Metadata for one message attachment. The decoded bytes are NOT held here — they
 * are fetched on demand by index so listing a message stays cheap.
 *
 * @param filename declared filename; may be blank for unnamed parts.
 * @param mimeType MIME type as `type/subtype` (e.g. `image/png`).
 * @param size     decoded size in bytes.
 */
data class Attachment(
    val filename: String,
    val mimeType: String,
    val size: Long,
)
