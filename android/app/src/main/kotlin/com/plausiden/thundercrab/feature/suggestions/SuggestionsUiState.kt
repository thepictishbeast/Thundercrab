// ============================================================================
// feature/suggestions/SuggestionsUiState.kt  (VIEWMODEL group)
// UI contract for the Suggestions screen. A flat data class (mirrors
// AccountSetupUiState) — not a sealed state — because the screen shows derived
// suggestions, saved rules, and the emitted Sieve simultaneously. No field is an
// Ffi* type; only DATA domain models (RuleSuggestion, ErrorKind) cross here.
// Spec §4 / VIEWMODEL contracts.
// ============================================================================
package com.plausiden.thundercrab.feature.suggestions

import com.plausiden.thundercrab.data.model.ErrorKind
import com.plausiden.thundercrab.data.model.RuleSuggestion

/**
 * State for the Suggestions screen.
 *
 * @param loading      true while the initial load (suggestions + rules + sieve) runs.
 * @param suggestions  derived rule suggestions awaiting acceptance.
 * @param savedRules   already-accepted/saved rules.
 * @param sieve        the emitted Sieve script for the saved rules, shown READ-ONLY.
 *                     Never pushed to a server (AVP-2 guardrail).
 * @param busyIds      ids of rules with an accept/delete action in flight (for per-row UI).
 * @param errorKind    mapped error category of the last failed action, or null.
 * @param errorMessage human-readable detail of the last failed action, or null.
 */
data class SuggestionsUiState(
    val loading: Boolean = true,
    val suggestions: List<RuleSuggestion> = emptyList(),
    val savedRules: List<RuleSuggestion> = emptyList(),
    val sieve: String = "",
    val busyIds: Set<String> = emptySet(),
    val errorKind: ErrorKind? = null,
    val errorMessage: String? = null,
)
