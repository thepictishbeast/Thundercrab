// ============================================================================
// feature/read/MessageReadViewModel.kt  (VIEWMODEL group)
// Reads the cached header (repo.headerFor) AND fetches the message body for
// display (repo.fetchBody — sanitized by the core). Optionally marks the
// message \Seen on open. Body content is display-only; it never feeds rules.
// ============================================================================
package com.plausiden.thundercrab.feature.read

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.plausiden.thundercrab.data.ThunderCrabRepository
import com.plausiden.thundercrab.data.model.ComposeDraft
import com.plausiden.thundercrab.data.model.OutboundAttachment
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

class MessageReadViewModel(
    private val repo: ThunderCrabRepository,
    private val folder: String,
    private val uid: Int,
) : ViewModel() {

    private val _uiState = MutableStateFlow(loadFromCache())
    val uiState: StateFlow<MessageReadUiState> = _uiState.asStateFlow()

    /** Emitted when Reply/Forward has built a draft; the screen hands it to the
     *  compose flow and calls [consumeDraft]. */
    private val _draftReady = MutableStateFlow<ComposeDraft?>(null)
    val draftReady: StateFlow<ComposeDraft?> = _draftReady.asStateFlow()

    /** True while a forward is fetching original attachment bytes. */
    private val _preparingForward = MutableStateFlow(false)
    val preparingForward: StateFlow<Boolean> = _preparingForward.asStateFlow()

    init {
        fetchBody()
    }

    /**
     * Reads the cached header synchronously (headerFor is non-suspend / no-throw).
     * Returns a `found = false` state when the header was not previously fetched.
     */
    private fun loadFromCache(): MessageReadUiState {
        val header = repo.headerFor(folder, uid) ?: return MessageReadUiState(found = false)
        return MessageReadUiState(
            from = header.from,
            subject = header.subject,
            headers = header.otherHeaders,
            found = true,
            readReceiptRequested = header.readReceiptRequested,
            bodyLoading = true,
        )
    }

    /** Fetch the message body (sanitized by the core) for display. */
    private fun fetchBody() {
        if (!_uiState.value.found) {
            repo.logUiEvent("MessageRead $folder/$uid: header NOT in cache (blank screen)")
            return
        }
        _uiState.update { it.copy(bodyLoading = true, bodyError = null) }
        viewModelScope.launch {
            repo.fetchBody(folder, uid).fold(
                onSuccess = { body ->
                    // Sizes/counts only — never body content.
                    repo.logUiEvent(
                        "MessageRead $folder/$uid: html=${body.htmlSanitized?.length ?: 0}ch " +
                            "plain=${body.plain?.length ?: 0}ch attachments=${body.attachments.size}",
                    )
                    _uiState.update {
                        it.copy(
                            bodyLoading = false,
                            bodyHtml = body.htmlSanitized,
                            bodyPlain = body.plain,
                            attachments = body.attachments,
                        )
                    }
                },
                onFailure = { e ->
                    repo.logUiEvent("MessageRead $folder/$uid: body FAILED: ${e.message}")
                    _uiState.update {
                        it.copy(
                            bodyLoading = false,
                            bodyError = e.message ?: "Couldn't load the message body.",
                        )
                    }
                },
            )
        }
    }

    /**
     * Fetch one attachment's decoded bytes (by its index in `attachments`). The
     * screen writes the bytes to a user-chosen file (Storage Access Framework),
     * so the IO stays in the UI layer and the ViewModel holds no Android Context.
     */
    suspend fun loadAttachmentBytes(index: Int): Result<ByteArray> =
        repo.fetchAttachment(folder, uid, index)

    /**
     * Build a reply draft from the loaded message and emit it via [draftReady].
     * Reply targets Reply-To if the sender set one, else From. Threads via
     * In-Reply-To = original Message-ID and the References chain. Carries NO
     * original attachments (a reply is new content, not a re-send). Read receipt
     * is left off by default.
     */
    fun reply() {
        val s = _uiState.value
        if (!s.found) return
        val messageId = ReplyForward.headerValue(s.headers, "message-id")
        _draftReady.value = ComposeDraft(
            to = ReplyForward.headerValue(s.headers, "reply-to") ?: s.from,
            subject = ReplyForward.replySubject(s.subject),
            body = ReplyForward.quotedReply(
                from = s.from,
                date = ReplyForward.headerValue(s.headers, "date").orEmpty(),
                body = s.bodyPlain.orEmpty(),
            ),
            inReplyTo = messageId,
            references = ReplyForward.referencesChain(
                ReplyForward.headerValue(s.headers, "references"),
                messageId,
            ),
        )
    }

    /**
     * Build a forward draft. Fetches each original attachment's bytes so the
     * forward carries them, then emits the draft via [draftReady]. Recipients
     * are left empty (the user chooses). Threading headers are omitted — a
     * forward starts a new thread.
     */
    fun forward() {
        val s = _uiState.value
        if (!s.found) return
        _preparingForward.value = true
        viewModelScope.launch {
            val carried = s.attachments.mapIndexedNotNull { index, a ->
                loadAttachmentBytes(index).getOrNull()?.let { bytes ->
                    OutboundAttachment(filename = a.filename, mimeType = a.mimeType, bytes = bytes)
                }
            }
            _preparingForward.value = false
            _draftReady.value = ComposeDraft(
                to = "",
                subject = ReplyForward.forwardSubject(s.subject),
                body = ReplyForward.forwardedBody(
                    from = s.from,
                    date = ReplyForward.headerValue(s.headers, "date").orEmpty(),
                    subject = s.subject,
                    to = ReplyForward.headerValue(s.headers, "to").orEmpty(),
                    body = s.bodyPlain.orEmpty(),
                ),
                inReplyTo = null,
                references = null,
                attachments = carried,
            )
        }
    }

    /** Clear the emitted draft after the screen has consumed it. */
    fun consumeDraft() {
        _draftReady.value = null
    }

    /**
     * Optionally mark the open message \Seen. Best-effort: failures are swallowed
     * because the read screen has no error surface for it.
     */
    fun markSeen() {
        if (!_uiState.value.found) return
        viewModelScope.launch {
            repo.setFlag(folder, uid, "\\Seen", true)
        }
    }

    companion object {
        fun factory(
            repo: ThunderCrabRepository,
            folder: String,
            uid: Int,
        ): ViewModelProvider.Factory =
            viewModelFactory {
                initializer { MessageReadViewModel(repo, folder, uid) }
            }
    }
}
