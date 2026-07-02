// ============================================================================
// data/model/OutboundAttachment.kt  (DATA group)
// A file the user chose to attach to an outgoing message. DATA-owned domain
// model — contains NO Ffi* type. Mapped to uniffi.thundercrab_ffi
// .FfiOutboundAttachment inside the Repository (spec §4). The UI resolves a
// picked content:// Uri into these owned bytes (via ContentResolver) so the
// ViewModel and Repository stay free of Android platform types.
// ============================================================================
package com.plausiden.thundercrab.data.model

/**
 * One outgoing attachment as chosen in the compose screen.
 *
 * @param filename display name shown to the recipient (Content-Disposition).
 * @param mimeType MIME type resolved from the content provider; the SMTP layer
 *                 falls back to `application/octet-stream` if it can't parse it.
 * @param bytes    the raw file bytes, already read off the Uri.
 *
 * Note: `bytes` makes structural `equals`/`hashCode` awkward (arrays compare by
 * reference), but attachments are only ever identified positionally in the
 * compose list, so reference identity is the intended behavior here.
 */
data class OutboundAttachment(
    val filename: String,
    val mimeType: String,
    val bytes: ByteArray,
)
