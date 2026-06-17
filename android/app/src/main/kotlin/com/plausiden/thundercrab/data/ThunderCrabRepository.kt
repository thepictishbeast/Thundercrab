// ============================================================================
// data/ThunderCrabRepository.kt  (DATA group)
// The SINGLE layer allowed to import uniffi.thundercrab_ffi.*
// Everything below the interface is the frozen implementation skeleton.
// Domain models (Folder, MessageHeader, AccountDraft, ConnectResult, ErrorKind)
// are DATA-owned and contain NO Ffi* type. See spec §4.
// ============================================================================
package com.plausiden.thundercrab.data

import com.plausiden.thundercrab.data.model.AccountDraft
import com.plausiden.thundercrab.data.model.ConnectResult
import com.plausiden.thundercrab.data.model.Folder
import com.plausiden.thundercrab.data.model.MessageHeader

/**
 * The app's only seam onto thundercrab-ffi. ViewModels depend on THIS, never on
 * uniffi.thundercrab_ffi.*. All Ffi* types are mapped to domain models here.
 * Frozen contract for the VIEWMODEL group. AVP-2: UNVERIFIED.
 */
interface ThunderCrabRepository {

    /** Prefill host/ports from the core. Blocking FFI; no-throw. Pure shape-map. */
    fun accountDraftFor(username: String): AccountDraft

    /** Connect + LOGIN. Stores the client internally. Never leaks ThunderCrabClient. */
    suspend fun connect(draft: AccountDraft, password: String): ConnectResult

    /** True once a live client is held. */
    fun isConnected(): Boolean

    /** List folders on the connected account. Requires a prior successful connect(). */
    suspend fun listFolders(): Result<List<Folder>>

    /**
     * Header-only fetch. Caches results per folder so MessageRead can read headers
     * without any body call. Never reads bodies.
     */
    suspend fun fetchHeaders(folder: String, limit: Int = 50): Result<List<MessageHeader>>

    /** Cached header lookup for the MessageRead screen. Null if not yet fetched. */
    fun headerFor(folder: String, uid: Int): MessageHeader?

    /**
     * Toggle a flag (e.g. "\\Seen", "\\Flagged"), then record a features-only
     * FfiFlagEvent (source = TOGGLE_FLAG). Header construction of the event is
     * internal (spec §5.3).
     */
    suspend fun setFlag(folder: String, uid: Int, flag: String, set: Boolean): Result<Unit>

    /**
     * Move a message, then record a features-only FfiFlagEvent (source = MANUAL_MOVE,
     * destination = toFolder).
     */
    suspend fun moveMessage(fromFolder: String, toFolder: String, uid: Int): Result<Unit>

    /** Best-effort logout (drops IMAP session) + close()/destroy() of the Rust handle. */
    suspend fun disconnect()

    // NOTE: fetchBody is DELIBERATELY ABSENT — no caller, no-body invariant (spec §5.1).
    // NOTE: send/pushSieve/saveRule/loadRules/deleteRule/previewSuggestions are out of P1 (spec §6).
}
