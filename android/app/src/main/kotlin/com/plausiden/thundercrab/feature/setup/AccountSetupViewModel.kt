// ============================================================================
// feature/setup/AccountSetupViewModel.kt  (VIEWMODEL group)
// Depends ONLY on ThunderCrabRepository + DATA domain models. Never imports
// uniffi.thundercrab_ffi.* and never catches FfiException — connect() returns a
// sealed ConnectResult, which we switch on. Spec §1 / §4 / VIEWMODEL contracts.
// ============================================================================
package com.plausiden.thundercrab.feature.setup

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.plausiden.thundercrab.data.ThunderCrabRepository
import com.plausiden.thundercrab.data.model.ConnectResult
import com.plausiden.thundercrab.data.model.ErrorKind
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

class AccountSetupViewModel(
    private val repo: ThunderCrabRepository,
) : ViewModel() {

    private val _uiState = MutableStateFlow(AccountSetupUiState())
    val uiState: StateFlow<AccountSetupUiState> = _uiState.asStateFlow()

    /**
     * Updates the email and recomputes the prefilled [AccountDraft] from the core
     * once the email looks addressable (contains '@'). `accountDraftFor` is a
     * blocking-but-cheap, no-throw Repository call — safe on the main thread.
     */
    fun onEmailChanged(email: String) {
        val draft = if (email.contains('@')) repo.accountDraftFor(email) else null
        _uiState.update { it.copy(email = email, draft = draft) }
    }

    fun onPasswordChanged(password: String) {
        _uiState.update { it.copy(password = password) }
    }

    /**
     * Attempts to connect + LOGIN via the Repository. Flips `connecting` while in
     * flight, then sets `connected` on success or `errorKind`/`errorMessage` on
     * failure. No host/port logic and no FFI types here — all of that lives in the
     * Repository / Rust core.
     */
    fun onConnect() {
        val state = _uiState.value
        val draft = state.draft
        if (draft == null) {
            _uiState.update {
                it.copy(
                    errorKind = ErrorKind.INVALID_INPUT,
                    errorMessage = "Enter a valid email address first.",
                )
            }
            return
        }
        if (state.connecting) return

        _uiState.update { it.copy(connecting = true, errorKind = null, errorMessage = null) }
        viewModelScope.launch {
            when (val result = repo.connect(draft, state.password)) {
                is ConnectResult.Connected ->
                    _uiState.update { it.copy(connecting = false, connected = true) }
                is ConnectResult.Failure ->
                    _uiState.update {
                        it.copy(
                            connecting = false,
                            errorKind = result.kind,
                            errorMessage = result.message,
                        )
                    }
            }
        }
    }

    /** Clears the error after the UI has surfaced it. */
    fun consumeError() {
        _uiState.update { it.copy(errorKind = null, errorMessage = null) }
    }

    companion object {
        fun factory(repo: ThunderCrabRepository): ViewModelProvider.Factory =
            viewModelFactory {
                initializer { AccountSetupViewModel(repo) }
            }
    }
}
