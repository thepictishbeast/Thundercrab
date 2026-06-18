// data/RuleSummary.kt (DATA group)
// The one piece of non-trivial pure logic in the data layer: turning a rule's
// canonical Action JSON into a short human label for the Suggestions UI. Pulled
// out of ThunderCrabRepositoryImpl as `internal` so the app's JVM unit tests can
// exercise it directly (no emulator, no FFI).
// AVP-2: UNVERIFIED — UNSAFE — nothing here is SHIP-DECISION.
package com.plausiden.thundercrab.data

import org.json.JSONObject

/**
 * Derive a short human summary from a rule's canonical Action JSON
 * (serde-tagged `{"kind":..}`, snake_case — see thundercrab-core crab_rule.rs).
 * Falls back to [fallback] (typically the rule's display name) on any parse
 * failure or unrecognized shape.
 *   file_into -> "Move to <folder>"
 *   set_flag  -> "Flag as <flag>"
 *   sequence  -> the first action's summary (+ " …" if more follow)
 */
internal fun summarizeRuleAction(actionJson: String, fallback: String): String = try {
    summarizeAction(JSONObject(actionJson)) ?: fallback
} catch (_: Exception) {
    fallback
}

private fun summarizeAction(obj: JSONObject): String? = when (obj.optString("kind")) {
    "file_into" -> obj.optString("folder").takeIf { it.isNotBlank() }?.let { "Move to $it" }
    "set_flag" -> obj.optString("flag").takeIf { it.isNotBlank() }?.let { "Flag as $it" }
    "sequence" -> {
        val actions = obj.optJSONArray("actions")
        val first = actions?.takeIf { it.length() > 0 }?.optJSONObject(0)?.let { summarizeAction(it) }
        when {
            first == null -> null
            (actions?.length() ?: 0) > 1 -> "$first …"
            else -> first
        }
    }
    else -> null
}
