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
import com.plausiden.thundercrab.data.model.DiagEvent
import com.plausiden.thundercrab.data.model.Folder
import com.plausiden.thundercrab.data.model.MessageBody
import com.plausiden.thundercrab.data.model.MessageHeader
import com.plausiden.thundercrab.data.model.OutboundAttachment
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

    /**
     * Server-side full-text search of `folder` for `term` (IMAP `SEARCH TEXT`,
     * headers + body), newest first. `term` is plain text — the core wraps it
     * safely. Read-only: searching never marks messages read. Caches the hits
     * per folder so the MessageRead screen can open a result without a re-fetch.
     */
    suspend fun search(folder: String, term: String): Result<List<MessageHeader>>

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

    // --- Folder management (IMAP CREATE/RENAME/DELETE/SUBSCRIBE) --------------

    /** Create a mailbox. `name` is the full server path (server hierarchy separator). */
    suspend fun createFolder(name: String): Result<Unit>

    /** Rename / move a mailbox (renaming a parent moves its children). */
    suspend fun renameFolder(from: String, to: String): Result<Unit>

    /** Delete a mailbox. The caller is responsible for confirming first. */
    suspend fun deleteFolder(name: String): Result<Unit>

    /** Subscribe (true) / unsubscribe (false) a mailbox. */
    suspend fun setSubscribed(name: String, subscribed: Boolean): Result<Unit>

    /**
     * Send an outbound message via SMTP. `password` is supplied per-call and
     * never retained (spec §5.2). `from` is the connected account; signature is
     * appended by the caller into `body`.
     */
    suspend fun sendMessage(
        password: String,
        to: List<String>,
        cc: List<String>,
        subject: String,
        body: String,
        /** Request a read receipt (RFC 8098) to the sending account. Opt-in. */
        readReceipt: Boolean = false,
        /** Files to attach; empty sends an unattached message unchanged. */
        attachments: List<OutboundAttachment> = emptyList(),
    ): Result<Unit>

    /**
     * Save a user-authored mail-routing rule (origin = USER). `whenJson` is
     * canonical MatchExpr JSON, `actionJson` canonical Action JSON. Built by the
     * graphical rule editor; stored in the local rule DB and emitted to Sieve
     * via [savedRulesSieve].
     */
    suspend fun saveUserRule(displayName: String, whenJson: String, actionJson: String): Result<Unit>

    // --- Diagnostics / telemetry (on-device, PII-free; see docs/TELEMETRY.md) -

    /** Whether on-device diagnostics collection is currently enabled. */
    fun telemetryEnabled(): Boolean

    /** Master switch for diagnostics collection (off = nothing recorded/sent). */
    fun setTelemetryEnabled(on: Boolean)

    /** Snapshot of recent diagnostic events (enumerated, PII-free). */
    fun diagnostics(): List<DiagEvent>

    /** Discard all buffered diagnostic events. */
    fun clearDiagnostics()

    /**
     * Best-effort upload of buffered diagnostics to the home endpoint (debug
     * builds). No-op when telemetry is disabled or there's nothing to send.
     * Clears the buffer on success. PII-free payload (see docs/TELEMETRY.md).
     */
    suspend fun uploadDiagnostics(): Result<Unit>

    /**
     * Record a UI breadcrumb ("MessageList INBOX: showed 0 rows") into the
     * on-device ring, so a diagnostics report can reconstruct what the user
     * SAW. Stays on-device; leaves only inside [sendDiagnosticsReport].
     */
    fun logUiEvent(line: String)

    /**
     * Build a diagnostics report (app/device context + diag events + the
     * on-device error/breadcrumb ring) and append it to the user's OWN
     * mailbox in the `ThunderCrab-Diagnostics` folder (IMAP APPEND — no
     * third-party endpoint). User-initiated only.
     */
    suspend fun sendDiagnosticsReport(note: String): Result<Unit>

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
     * Serialize every saved rule to a JSON array (for cross-device sync via the
     * personalization blob). The `FfiCrabRule` fields are already JSON-friendly
     * (the match/action ASTs are canonical JSON strings).
     */
    suspend fun exportRulesJson(): Result<String>

    /**
     * Upsert the rules in `json` (from a synced blob) into the local store. A
     * merge, not a replace — locally-only rules are never dropped, so syncing
     * can't lose a rule.
     */
    suspend fun importRulesJson(json: String): Result<Unit>

    /**
     * Pull the synced rules from the user's mailbox ("rules" METADATA entry) and
     * upsert them locally. Rules push themselves the instant they change; this is
     * the pull side (call on app/settings open). No-op if not connected.
     */
    suspend fun syncRulesDown(): Result<Unit>

    /**
     * Emit the personal Sieve script (RFC 5228) for the currently saved rules:
     * loads the rules then renders them. Pure preview — NEVER pushed to a server
     * (AVP-2 guardrail). Returns the script text for read-only display.
     */
    suspend fun savedRulesSieve(): Result<String>

    /**
     * Fetch one message's body for display. Returns the plain-text part and the
     * HTML part ALREADY SANITIZED by the core (scripts + all remote content
     * removed — safe for a locked-down WebView). Uses BODY.PEEK, so reading a
     * message does not itself mark it \Seen.
     *
     * Display-only: body content is never fed to the rules / suggestions /
     * ledger spine. (Supersedes the former no-body invariant, lifted on the
     * owner's "render received mail" decision.)
     */
    suspend fun fetchBody(folder: String, uid: Int): Result<MessageBody>

    /**
     * Fetch the decoded bytes of one attachment by its index in the message's
     * [MessageBody.attachments] list. Fetched on demand (separate from the body)
     * so listing a message stays cheap regardless of attachment size.
     */
    suspend fun fetchAttachment(folder: String, uid: Int, index: Int): Result<ByteArray>

    /**
     * Cross-device personalization sync via IMAP METADATA (the user's own
     * mailbox — no third-party sync server). [getPersonalization] returns the
     * stored JSON blob (null if none / server lacks METADATA); [setPersonalization]
     * stores it. The schema is the app's; the seam is opaque.
     */
    suspend fun getPersonalization(): Result<String?>
    suspend fun setPersonalization(json: String): Result<Unit>
}
