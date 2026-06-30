// ============================================================================
// feature/messages/MessageListViewModel.kt  (VIEWMODEL group)
// Fetches header-only rows and performs flag/move actions via the Repository.
// The features-only FfiFlagEvent emission is INTERNAL to the Repository — this
// ViewModel only calls setFlag / moveMessage. Flag strings are escaped backslash
// literals ("\\Seen" / "\\Flagged"). Spec §2 / §5.1 / §5.3 / VIEWMODEL contracts.
// ============================================================================
package com.plausiden.thundercrab.feature.messages

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.plausiden.thundercrab.data.RepositoryError
import com.plausiden.thundercrab.data.ThunderCrabRepository
import com.plausiden.thundercrab.data.model.ErrorKind
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

class MessageListViewModel(
    private val repo: ThunderCrabRepository,
    private val folder: String,          // from nav arg, passed via factory(repo, folder)
) : ViewModel() {

    private val _uiState = MutableStateFlow(MessageListUiState(folder = folder))
    val uiState: StateFlow<MessageListUiState> = _uiState.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        _uiState.update { it.copy(loading = true, query = "", isSearchResult = false) }
        viewModelScope.launch {
            repo.fetchHeaders(folder, MESSAGE_LIMIT).fold(
                onSuccess = { headers ->
                    _uiState.update { it.copy(loading = false, messages = headers) }
                },
                onFailure = { e -> applyError(e) },
            )
        }
    }

    /** Server-side full-text search of this folder. Blank term restores the listing. */
    fun onSearch(term: String) {
        val trimmed = term.trim()
        if (trimmed.isEmpty()) {
            refresh()
            return
        }
        _uiState.update { it.copy(loading = true, query = trimmed) }
        viewModelScope.launch {
            repo.search(folder, trimmed).fold(
                onSuccess = { hits ->
                    _uiState.update {
                        it.copy(loading = false, messages = hits, isSearchResult = true)
                    }
                },
                onFailure = { e -> applyError(e) },
            )
        }
    }

    /** Clear the search and return to the full folder listing. */
    fun clearSearch() = refresh()

    /** Toggle the \Seen flag, then refresh on success. */
    fun onToggleSeen(uid: Int, seen: Boolean) = launchFlag(uid, "\\Seen", seen)

    /** Toggle the \Flagged flag, then refresh on success. */
    fun onToggleFlagged(uid: Int, flagged: Boolean) = launchFlag(uid, "\\Flagged", flagged)

    /** Move a message to another folder, then refresh on success. */
    fun onMove(uid: Int, toFolder: String) {
        viewModelScope.launch {
            repo.moveMessage(folder, toFolder, uid).fold(
                onSuccess = { refresh() },
                onFailure = { e -> applyError(e) },
            )
        }
    }

    fun consumeError() {
        _uiState.update { it.copy(errorKind = null, errorMessage = null) }
    }

    private fun launchFlag(uid: Int, flag: String, set: Boolean) {
        viewModelScope.launch {
            repo.setFlag(folder, uid, flag, set).fold(
                onSuccess = { refresh() },
                onFailure = { e -> applyError(e) },
            )
        }
    }

    private fun applyError(e: Throwable) {
        val kind = (e as? RepositoryError)?.kind ?: ErrorKind.UNKNOWN
        _uiState.update {
            it.copy(
                loading = false,
                errorKind = kind,
                errorMessage = e.message ?: "Operation failed.",
            )
        }
    }

    companion object {
        private const val MESSAGE_LIMIT = 50

        fun factory(repo: ThunderCrabRepository, folder: String): ViewModelProvider.Factory =
            viewModelFactory {
                initializer { MessageListViewModel(repo, folder) }
            }
    }
}
