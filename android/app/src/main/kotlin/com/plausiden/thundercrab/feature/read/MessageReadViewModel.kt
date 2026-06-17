// ============================================================================
// feature/read/MessageReadViewModel.kt  (VIEWMODEL group)
// Reads ONLY the cached header (repo.headerFor) populated by the message list.
// It MUST NOT call fetchBody (not even on the Repository surface). Optionally
// marks the message \Seen on open. Spec §2 / §5.1 / VIEWMODEL contracts.
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
import kotlinx.coroutines.launch

class MessageReadViewModel(
    private val repo: ThunderCrabRepository,
    private val folder: String,
    private val uid: Int,
) : ViewModel() {

    private val _uiState = MutableStateFlow(loadFromCache())
    val uiState: StateFlow<MessageReadUiState> = _uiState.asStateFlow()

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
        )
    }

    /**
     * Optionally mark the open message \Seen. Best-effort: failures are swallowed
     * here because the read screen has no error surface (the body is a fixed
     * disabled state and the headers are already cached). Spec §2.
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
