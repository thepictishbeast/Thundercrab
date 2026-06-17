// ============================================================================
// data/model/MessageHeader.kt  (DATA group)
// A header-only message summary. DATA-owned domain model — NO Ffi* type.
// Mapped from uniffi.thundercrab_ffi.FfiHeaders inside the Repository (spec §4).
// HEADER-ONLY by construction: there is intentionally NO body field here, and
// the Repository never reads a body (no-body invariant, spec §5.1).
// ============================================================================
package com.plausiden.thundercrab.data.model

/**
 * A single message, headers only.
 *
 * @param uid          IMAP UID (FFI UInt narrowed to Int at the seam).
 * @param folder       server-side folder path the message lives in.
 * @param from         `From:` header value.
 * @param subject      `Subject:` header value.
 * @param otherHeaders additional captured headers as (name, value) pairs. Header
 *                     names arrive lowercased from the core; consumers must compare
 *                     case-insensitively.
 */
data class MessageHeader(
    val uid: Int,
    val folder: String,
    val from: String,
    val subject: String,
    val otherHeaders: List<Pair<String, String>>,
)
