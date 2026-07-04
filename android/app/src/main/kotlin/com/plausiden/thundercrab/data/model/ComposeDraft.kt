// ============================================================================
// data/model/ComposeDraft.kt  (DATA group)
// A prefilled compose draft handed from the read screen (Reply/Forward) to the
// compose screen via a one-shot holder on AppContainer. DATA-owned — NO Ffi*
// type. Carries the derived recipients/subject/body plus the RFC 5322 threading
// IDs; forward drafts also carry the original attachments (bytes fetched up
// front). A fresh compose uses no draft (null).
// ============================================================================
package com.plausiden.thundercrab.data.model

/**
 * A prefilled outgoing draft.
 *
 * @param to          recipients for the To field (comma-joinable string).
 * @param cc          recipients for the Cc field.
 * @param subject     already prefixed (Re:/Fwd:) subject.
 * @param body        prefilled body (quoted reply or forwarded block).
 * @param inReplyTo   original Message-ID (with angle brackets) for a reply, or
 *                    null (forwards start a new thread).
 * @param references  the References chain to emit, or null.
 * @param attachments original attachments to carry (forward), else empty.
 */
data class ComposeDraft(
    val to: String = "",
    val cc: String = "",
    val subject: String = "",
    val body: String = "",
    val inReplyTo: String? = null,
    val references: String? = null,
    val attachments: List<OutboundAttachment> = emptyList(),
)
