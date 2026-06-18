// ============================================================================
// data/model/RuleSuggestion.kt  (DATA group)
// A derived/saved sorting rule as surfaced to the UI. DATA-owned domain model —
// contains NO Ffi* type. Mapped from uniffi.thundercrab_ffi.FfiCrabRule inside
// the Repository (spec §4). The full FfiCrabRule (when/action JSON, score,
// origin, …) is intentionally NOT exposed here: the UI only needs an id to act
// on and human-readable text to show. The Repository keeps the full rule behind
// the firewall and looks it up by [id] on accept.
// ============================================================================
package com.plausiden.thundercrab.data.model

/**
 * A single sorting rule for the Suggestions screen — either a derived
 * suggestion awaiting acceptance or an already-saved rule.
 *
 * @param id          stable rule id (the key used to accept/delete).
 * @param displayName human-readable rule name from the core.
 * @param summary     short human summary of what the rule does, derived by the
 *                    Repository from the rule's action (e.g. "Move to Promotions").
 *                    Falls back to [displayName] when the action can't be parsed.
 */
data class RuleSuggestion(
    val id: String,
    val displayName: String,
    val summary: String,
)
