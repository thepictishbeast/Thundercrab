// ============================================================================
// data/ThunderCrabRepositoryImpl.kt  (DATA group)
// The ONLY file (besides the interface's import-free signatures) that touches
// uniffi.thundercrab_ffi.*. All Ffi* types are quarantined here and mapped to
// DATA-owned domain models before they cross back out (the firewall, spec §4).
// AVP-2: UNVERIFIED — UNSAFE — nothing here is SHIP-DECISION.
// ============================================================================
package com.plausiden.thundercrab.data

import com.plausiden.thundercrab.data.model.AccountDraft
import com.plausiden.thundercrab.data.model.ConnectResult
import com.plausiden.thundercrab.data.model.ErrorKind
import com.plausiden.thundercrab.data.model.Folder
import com.plausiden.thundercrab.data.model.MessageHeader
import com.plausiden.thundercrab.data.model.RuleSuggestion
import java.security.MessageDigest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
// The ONE import block of the generated surface, isolated to this file:
import uniffi.thundercrab_ffi.FfiAccountConfig
import uniffi.thundercrab_ffi.FfiCrabRule
import uniffi.thundercrab_ffi.FfiException
import uniffi.thundercrab_ffi.FfiFlagEvent
import uniffi.thundercrab_ffi.FfiFlagSource
import uniffi.thundercrab_ffi.FfiHeaders
import uniffi.thundercrab_ffi.FfiFolder
import uniffi.thundercrab_ffi.FfiRuleOrigin
import uniffi.thundercrab_ffi.ThunderCrabClient
import uniffi.thundercrab_ffi.connect as ffiConnect
import uniffi.thundercrab_ffi.deleteRule as ffiDeleteRule
import uniffi.thundercrab_ffi.loadRules as ffiLoadRules
import uniffi.thundercrab_ffi.plausidenAccountConfig
import uniffi.thundercrab_ffi.previewSuggestions as ffiPreviewSuggestions
import uniffi.thundercrab_ffi.recordFlagEvent
import uniffi.thundercrab_ffi.rulesToSieve as ffiRulesToSieve
import uniffi.thundercrab_ffi.saveRule as ffiSaveRule

/**
 * @param dbPath absolute SQLite path, injected by AppContainer
 *               (= "${filesDir.absolutePath}/thundercrab.db"). Spec §1.1.
 *
 * Client lifecycle (spec §1.2): at most one connected client for the whole app
 * lifetime, held in a Mutex-guarded nullable field. connect() replaces it;
 * disconnect() logs out + close()s the Rust handle.
 *
 * The dbPath SQLite store holds only rules + features-only flag events — no
 * bodies, no passwords (spec §5.2).
 */
class ThunderCrabRepositoryImpl(
    private val dbPath: String,
) : ThunderCrabRepository {

    private val clientLock = Mutex()
    private var client: ThunderCrabClient? = null
    // Per-folder header cache feeding MessageRead (no-body invariant, spec §5.1/§2).
    private val headerCache = mutableMapOf<String, List<MessageHeader>>()
    // Last preview's full rules, keyed by id, so acceptSuggestion() can persist
    // the EXACT previewed rule without re-deriving (re-derivation is racy — a
    // suggestion can vanish between preview and accept). Populated by
    // previewSuggestions(); read by acceptSuggestion(). Ffi* stays in this file.
    private val suggestionCache = mutableMapOf<String, FfiCrabRule>()

    override fun accountDraftFor(username: String): AccountDraft {
        // plausidenAccountConfig is BLOCKING + no-throw. Pure shape-map.
        val c: FfiAccountConfig = plausidenAccountConfig(username)
        return AccountDraft(
            imapHost = c.imapHost,
            imapPort = c.imapPort.toString(),   // UShort -> form String
            smtpHost = c.smtpHost,
            smtpPort = c.smtpPort.toString(),
            sievePort = c.sievePort.toString(),
            username = c.username,
        )
    }

    override suspend fun connect(draft: AccountDraft, password: String): ConnectResult =
        withContext(Dispatchers.IO) {
            val cfg = draft.toFfi() ?: return@withContext ConnectResult.Failure(
                ErrorKind.INVALID_INPUT, "Host/port fields are invalid."
            )
            try {
                val newClient = ffiConnect(cfg, password) // suspend top-level free fn
                clientLock.withLock {
                    client?.runCatching { close() } // free any prior handle
                    client = newClient
                    headerCache.clear()
                }
                ConnectResult.Connected
            } catch (e: FfiException) {
                ConnectResult.Failure(e.toErrorKind(), e.messageText())
            }
        }

    override fun isConnected(): Boolean = client != null

    override suspend fun listFolders(): Result<List<Folder>> = guarded { c ->
        c.listFolders().map { it.toDomain() }
    }

    override suspend fun fetchHeaders(folder: String, limit: Int): Result<List<MessageHeader>> =
        guarded { c ->
            val headers = c.fetchHeaders(folder, limit.toUInt()).map { it.toDomain() }
            headerCache[folder] = headers
            headers
        }

    override fun headerFor(folder: String, uid: Int): MessageHeader? =
        headerCache[folder]?.firstOrNull { it.uid == uid }

    override suspend fun setFlag(folder: String, uid: Int, flag: String, set: Boolean): Result<Unit> =
        guarded { c ->
            c.setFlag(folder, uid.toUInt(), flag, set)
            recordEvent(folder, uid, FfiFlagSource.TOGGLE_FLAG, destination = folder)
        }

    override suspend fun moveMessage(fromFolder: String, toFolder: String, uid: Int): Result<Unit> =
        guarded { c ->
            c.moveMessage(fromFolder, toFolder, uid.toUInt())
            recordEvent(fromFolder, uid, FfiFlagSource.MANUAL_MOVE, destination = toFolder)
        }

    override suspend fun disconnect() = withContext(Dispatchers.IO) {
        clientLock.withLock {
            client?.let { c ->
                c.logout()                 // no-throw, best-effort
                c.runCatching { close() }  // free Rust Arc
            }
            client = null
            headerCache.clear()
        }
    }

    // --- Suggestions (local rule store; no IMAP client required) -------------

    override suspend fun previewSuggestions(minObs: Long, dominance: Double): Result<List<RuleSuggestion>> =
        ioCatching {
            val rules = ffiPreviewSuggestions(dbPath, minObs, dominance)
            // Refresh the accept cache to exactly this preview's rules.
            suggestionCache.clear()
            rules.forEach { suggestionCache[it.id] = it }
            rules.map { it.toDomain() }
        }

    override suspend fun acceptSuggestion(id: String): Result<Unit> = ioCatching {
        val rule = suggestionCache[id]
            ?: throw FfiException.InvalidInput("Suggestion no longer available; refresh and try again.")
        // The user explicitly accepted this rule -> mark provenance USER.
        ffiSaveRule(dbPath, rule.copy(origin = FfiRuleOrigin.USER))
    }

    override suspend fun loadSavedRules(): Result<List<RuleSuggestion>> = ioCatching {
        ffiLoadRules(dbPath).map { it.toDomain() }
    }

    override suspend fun deleteSavedRule(id: String): Result<Boolean> = ioCatching {
        ffiDeleteRule(dbPath, id)
    }

    override suspend fun savedRulesSieve(): Result<String> = ioCatching {
        // Load the saved rules then render — rulesToSieve needs the full FfiCrabRule.
        // READ-ONLY: the script is returned for display, never pushed (AVP-2).
        val rules = ffiLoadRules(dbPath)
        ffiRulesToSieve(rules)
    }

    // --- internals -----------------------------------------------------------

    /** Run a block with the live client on IO; map FfiException -> Result.failure. */
    private suspend fun <T> guarded(block: suspend (ThunderCrabClient) -> T): Result<T> =
        withContext(Dispatchers.IO) {
            val c = client ?: return@withContext Result.failure(
                IllegalStateException("Not connected")
            )
            try { Result.success(block(c)) }
            catch (e: FfiException) { Result.failure(RepositoryError(e.toErrorKind(), e.messageText())) }
        }

    /**
     * Run a block on IO without the IMAP-client gate; map FfiException ->
     * Result.failure. Used by the local-rule-store (Suggestions) calls, which
     * take dbPath and must work regardless of IMAP connection state.
     */
    private suspend fun <T> ioCatching(block: suspend () -> T): Result<T> =
        withContext(Dispatchers.IO) {
            try { Result.success(block()) }
            catch (e: FfiException) { Result.failure(RepositoryError(e.toErrorKind(), e.messageText())) }
        }

    /** Build + record the features-only FfiFlagEvent. Seam-logic, spec §5.3. */
    private fun recordEvent(folder: String, uid: Int, source: FfiFlagSource, destination: String) {
        val h = headerFor(folder, uid) ?: return
        val key = (h.headerValue("Message-ID") ?: "$folder:$uid").toByteArray()
        val hash = MessageDigest.getInstance("SHA-256").digest(key) // 32 bytes (placeholder, NOT blake3)
        val ev = FfiFlagEvent(
            messageHash = hash,
            source = source,
            destination = destination,
            fromDomainWithAt = senderDomainWithAt(h.from), // robust parse; "" if unparseable
            listId = h.headerValue("List-Id"),
            hasListUnsubscribe = h.headerValue("List-Unsubscribe") != null,
            subjectTokens = h.subject.lowercase().split(Regex("\\W+")).filter { it.isNotBlank() },
            priorityHigh = h.headerValue("Importance")?.equals("high", true) == true ||
                h.headerValue("X-Priority")?.trim()?.firstOrNull() in listOf('1', '2'),
            observedAt = System.currentTimeMillis() / 1000, // UTC epoch seconds (Long)
        )
        recordFlagEvent(dbPath, ev) // BLOCKING; already on Dispatchers.IO via guarded{}
    }

    // --- Ffi* -> domain mappers (the firewall) -------------------------------
    private fun FfiFolder.toDomain() =
        Folder(name = name, specialUse = specialUse, messages = messages.toLong(), unseen = unseen.toLong())

    private fun FfiCrabRule.toDomain() = RuleSuggestion(
        id = id,
        displayName = displayName,
        summary = summarizeRuleAction(actionJson, displayName), // pure helper in RuleSummary.kt
    )

    private fun FfiHeaders.toDomain() = MessageHeader(
        uid = uid.toInt(), folder = folder, from = from, subject = subject,
        otherHeaders = otherHeaders.map { it.name to it.value },
    )

    private fun AccountDraft.toFfi(): FfiAccountConfig? = try {
        FfiAccountConfig(
            imapHost = imapHost, imapPort = imapPort.toUShort(),
            smtpHost = smtpHost, smtpPort = smtpPort.toUShort(),
            sievePort = sievePort.toUShort(), username = username,
        )
    } catch (_: NumberFormatException) { null }

    private fun FfiException.messageText(): String = when (this) {
        is FfiException.Transport -> detail
        is FfiException.Auth -> detail
        is FfiException.Protocol -> detail
        is FfiException.NotImplemented -> detail
        is FfiException.InvalidInput -> detail
        else -> message ?: "Unknown FFI error"
    }

    private fun FfiException.toErrorKind(): ErrorKind = when (this) {
        is FfiException.Transport -> ErrorKind.TRANSPORT
        is FfiException.Auth -> ErrorKind.AUTH
        is FfiException.Protocol -> ErrorKind.PROTOCOL
        is FfiException.NotImplemented -> ErrorKind.NOT_IMPLEMENTED
        is FfiException.InvalidInput -> ErrorKind.INVALID_INPUT
        else -> ErrorKind.UNKNOWN
    }
}

/** Carries the mapped ErrorKind out of guarded{} Result.failure. */
class RepositoryError(val kind: ErrorKind, override val message: String) : Exception(message)

/** convenience accessor used by recordEvent */
private fun MessageHeader.headerValue(name: String): String? =
    otherHeaders.firstOrNull { it.first.equals(name, ignoreCase = true) }?.second
