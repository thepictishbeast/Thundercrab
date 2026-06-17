// ============================================================================
// feature/setup/AccountSetupUiState.kt  (VIEWMODEL group)
// Frozen UI contract for the UI group. No field is an Ffi* type; only DATA
// domain models (AccountDraft, ErrorKind) cross this boundary. Spec §4 / §5.2.
// ============================================================================
package com.plausiden.thundercrab.feature.setup

import com.plausiden.thundercrab.data.model.AccountDraft
import com.plausiden.thundercrab.data.model.ErrorKind

/**
 * State for the Account Setup screen.
 *
 * The [password] lives ONLY in this transient in-memory state during setup; it
 * is never persisted (spec §5.2). [draft] is prefilled host/ports derived from
 * the core via the Repository once a valid email is entered.
 */
data class AccountSetupUiState(
    val email: String = "",
    val password: String = "",          // in-memory only; never persisted (spec §5.2)
    val draft: AccountDraft? = null,     // prefilled host/ports once email is entered
    val connecting: Boolean = false,
    val errorKind: ErrorKind? = null,
    val errorMessage: String? = null,
    val connected: Boolean = false,      // UI observes -> navigate to "folders"
)
