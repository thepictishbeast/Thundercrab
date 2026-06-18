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
import com.plausiden.thundercrab.data.model.RuleSuggestion

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

    // --- Suggestions (local rule store; no IMAP session required) ------------
    // These operate on the local dbPath rule/event store, NOT on the IMAP
    // client, so they work whether or not connect() has succeeded. They preview
    // + accept (save) derived rules and emit a READ-ONLY Sieve script. They MUST
    // NOT push to any server: there is deliberately no pushSieve method here
    // (AVP-2 guardrail, spec §6).

    /**
     * Read-only preview of derived rule suggestions from the local flag-event
     * store. Blocking FFI run on IO; maps FfiException -> RepositoryError.
     * The full underlying rules are cached internally, keyed by id, so a later
     * [acceptSuggestion] can save the exact previewed rule.
     *
     * @param minObs    drop domain/destination pairs below this observation count.
     * @param dominance the destination must account for at least this fraction
     *                  (0.0..=1.0) of the domain's events.
     */
    suspend fun previewSuggestions(minObs: Long = 3L, dominance: Double = 0.7): Result<List<RuleSuggestion>>

    /**
     * Accept a previewed suggestion by id: persists the full underlying rule
     * (origin marked USER) into the local store via the core. Fails with an
     * INVALID_INPUT RepositoryError if the id is no longer in the preview cache.
     */
    suspend fun acceptSuggestion(id: String): Result<Unit>

    /** Load every saved rule from the local store, score-desc then id-asc. */
    suspend fun loadSavedRules(): Result<List<RuleSuggestion>>

    /** Delete a saved rule by id. Returns true if a rule was removed. */
    suspend fun deleteSavedRule(id: String): Result<Boolean>

    /**
     * Emit the personal Sieve script (RFC 5228) for the currently saved rules:
     * loads the rules then renders them. Pure preview — NEVER pushed to a server
     * (AVP-2 guardrail). Returns the script text for read-only display.
     */
    suspend fun savedRulesSieve(): Result<String>

    // NOTE: fetchBody is DELIBERATELY ABSENT — no caller, no-body invariant (spec §5.1).
    // NOTE: pushSieve/sendMessage remain out of scope here (AVP-2; no live push).
}
