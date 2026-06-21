// ============================================================================
// feature/rules/RuleEditorViewModel.kt  (VIEWMODEL group)
// Saves a user-authored mail-routing rule via the Repository (origin = USER).
// The screen builds canonical MatchExpr/Action JSON; this VM just persists it.
// No Ffi* here. AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.rules

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

class RuleEditorViewModel(
    private val repo: ThunderCrabRepository,
) : ViewModel() {

    private val _saved = MutableStateFlow(false)
    val saved: StateFlow<Boolean> = _saved.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    fun save(displayName: String, whenJson: String, actionJson: String) {
        viewModelScope.launch {
            repo.saveUserRule(displayName, whenJson, actionJson).fold(
                onSuccess = { _saved.value = true },
                onFailure = { e -> _error.value = e.message ?: "Couldn't save rule." },
            )
        }
    }

    fun consumeError() {
        _error.value = null
    }

    companion object {
        fun factory(repo: ThunderCrabRepository): ViewModelProvider.Factory =
            viewModelFactory {
                initializer { RuleEditorViewModel(repo) }
            }
    }
}
