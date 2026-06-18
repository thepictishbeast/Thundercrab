// ============================================================================
// feature/suggestions/SuggestionsViewModel.kt  (VIEWMODEL group)
// Drives the Suggestions screen via the Repository. All Repository calls return
// Result<T>; on failure the throwable is a RepositoryError carrying an ErrorKind.
// We never import uniffi.thundercrab_ffi.* and never catch FfiException. Blocking
// FFI runs on Dispatchers.IO inside the Repository. Spec §1 / §4.
//
// AVP-2 guardrail: this VM previews + accepts (saves) rules and loads the emitted
// Sieve for READ-ONLY display. There is no push action and no Repository push
// method to call.
// ============================================================================
package com.plausiden.thundercrab.feature.suggestions

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

class SuggestionsViewModel(
    private val repo: ThunderCrabRepository,
) : ViewModel() {

    private val _uiState = MutableStateFlow(SuggestionsUiState())
    val uiState: StateFlow<SuggestionsUiState> = _uiState.asStateFlow()

    init {
        refresh()
    }

    /** Reload suggestions + saved rules + the emitted Sieve preview. */
    fun refresh() {
        _uiState.update { it.copy(loading = true, errorKind = null, errorMessage = null) }
        viewModelScope.launch {
            val suggestionsResult = repo.previewSuggestions()
            val rulesResult = repo.loadSavedRules()
            val sieveResult = repo.savedRulesSieve()

            // Surface the first failure (if any) but still apply whatever loaded.
            val failure = sequenceOf(suggestionsResult, rulesResult, sieveResult)
                .firstOrNull { it.isFailure }
                ?.exceptionOrNull()

            _uiState.update {
                it.copy(
                    loading = false,
                    suggestions = suggestionsResult.getOrDefault(it.suggestions),
                    savedRules = rulesResult.getOrDefault(it.savedRules),
                    sieve = sieveResult.getOrDefault(it.sieve),
                    errorKind = failure?.let(::kindOf),
                    errorMessage = failure?.let { e -> e.message ?: "Failed to load suggestions." },
                )
            }
        }
    }

    /** Accept (save) a suggestion, then refresh saved rules + the Sieve preview. */
    fun accept(id: String) {
        if (id in _uiState.value.busyIds) return
        markBusy(id, busy = true)
        viewModelScope.launch {
            repo.acceptSuggestion(id).fold(
                onSuccess = { reloadSavedAndSieve(removeSuggestionId = id) },
                onFailure = { e -> fail(e); markBusy(id, busy = false) },
            )
        }
    }

    /** Delete a saved rule, then refresh saved rules + the Sieve preview. */
    fun delete(id: String) {
        if (id in _uiState.value.busyIds) return
        markBusy(id, busy = true)
        viewModelScope.launch {
            repo.deleteSavedRule(id).fold(
                onSuccess = { reloadSavedAndSieve(removeSuggestionId = null) },
                onFailure = { e -> fail(e); markBusy(id, busy = false) },
            )
        }
    }

    /** Clear the error after the UI has surfaced it. */
    fun consumeError() {
        _uiState.update { it.copy(errorKind = null, errorMessage = null) }
    }

    // --- internals -----------------------------------------------------------

    /** Reload saved rules + Sieve after a mutation; optionally drop an accepted suggestion row. */
    private suspend fun reloadSavedAndSieve(removeSuggestionId: String?) {
        val rulesResult = repo.loadSavedRules()
        val sieveResult = repo.savedRulesSieve()
        val failure = sequenceOf(rulesResult, sieveResult)
            .firstOrNull { it.isFailure }
            ?.exceptionOrNull()
        _uiState.update { s ->
            s.copy(
                suggestions = if (removeSuggestionId != null) {
                    s.suggestions.filterNot { it.id == removeSuggestionId }
                } else {
                    s.suggestions
                },
                savedRules = rulesResult.getOrDefault(s.savedRules),
                sieve = sieveResult.getOrDefault(s.sieve),
                busyIds = s.busyIds - setOfNotNull(removeSuggestionId),
                errorKind = failure?.let(::kindOf) ?: s.errorKind,
                errorMessage = failure?.let { e -> e.message ?: "Failed to refresh rules." } ?: s.errorMessage,
            )
        }
    }

    private fun markBusy(id: String, busy: Boolean) {
        _uiState.update {
            it.copy(busyIds = if (busy) it.busyIds + id else it.busyIds - id)
        }
    }

    private fun fail(e: Throwable) {
        _uiState.update {
            it.copy(errorKind = kindOf(e), errorMessage = e.message ?: "Action failed.")
        }
    }

    private fun kindOf(e: Throwable): ErrorKind =
        (e as? RepositoryError)?.kind ?: ErrorKind.UNKNOWN

    companion object {
        fun factory(repo: ThunderCrabRepository): ViewModelProvider.Factory =
            viewModelFactory {
                initializer { SuggestionsViewModel(repo) }
            }
    }
}
