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
        if (!_uiState.value.found) return
        _uiState.update { it.copy(bodyLoading = true, bodyError = null) }
        viewModelScope.launch {
            repo.fetchBody(folder, uid).fold(
                onSuccess = { body ->
                    _uiState.update {
                        it.copy(
                            bodyLoading = false,
                            bodyHtml = body.htmlSanitized,
                            bodyPlain = body.plain,
                        )
                    }
                },
                onFailure = { e ->
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
