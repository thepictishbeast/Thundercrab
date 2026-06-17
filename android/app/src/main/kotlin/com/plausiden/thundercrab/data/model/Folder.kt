// ============================================================================
// data/model/Folder.kt  (DATA group)
// A folder summary. DATA-owned domain model — contains NO Ffi* type.
// Mapped from uniffi.thundercrab_ffi.FfiFolder inside the Repository (spec §4).
// ============================================================================
package com.plausiden.thundercrab.data.model

/**
 * A mailbox folder as surfaced to the UI.
 *
 * @param name        folder name as the server presents it.
 * @param specialUse  IMAP special-use marker (`\Sent`, `\Drafts`, …) or null.
 * @param messages    total message count (FFI ULong widened to Long for the UI).
 * @param unseen      unread count (FFI ULong widened to Long for the UI).
 */
data class Folder(
    val name: String,
    val specialUse: String?,
    val messages: Long,
    val unseen: Long,
)
