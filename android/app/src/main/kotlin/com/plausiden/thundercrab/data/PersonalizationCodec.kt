// ============================================================================
// data/PersonalizationCodec.kt  (DATA group)
// Serializes the cross-device personalization blob synced via IMAP METADATA.
// Versioned + forward-compatible: unknown fields are ignored and missing ones
// fall back to defaults, so older and newer clients interoperate. Grow it by
// adding fields — never a protocol change. Uses framework org.json (no dep).
// ============================================================================
package com.plausiden.thundercrab.data

import org.json.JSONArray
import org.json.JSONObject

object PersonalizationCodec {
    private const val VERSION = 1

    /** Serialize the synced personalization (appearance + signature + rules) into the blob. */
    fun encode(appearance: AppearancePrefs, signature: String, rulesJson: String): String =
        JSONObject()
            .put("v", VERSION)
            .put("themeMode", appearance.themeMode.name)
            .put("amoled", appearance.amoled)
            .put("dynamicColor", appearance.dynamicColor)
            .put("signature", signature)
            .put("rules", JSONArray(rulesJson))
            .toString()

    /** The synced rules as a JSON-array string (for the repo to upsert), or null. */
    fun decodeRulesJson(json: String): String? = try {
        JSONObject(json).optJSONArray("rules")?.toString()
    } catch (_: Exception) {
        null
    }

    /** The synced signature, or null if the blob has none. */
    fun decodeSignature(json: String): String? = try {
        JSONObject(json).takeIf { it.has("signature") }?.getString("signature")
    } catch (_: Exception) {
        null
    }

    /** Parse the appearance from a synced blob, or null if it's unusable. */
    fun decodeAppearance(json: String): AppearancePrefs? = try {
        val o = JSONObject(json)
        AppearancePrefs(
            themeMode = runCatching { ThemeMode.valueOf(o.optString("themeMode", "SYSTEM")) }
                .getOrDefault(ThemeMode.SYSTEM),
            amoled = o.optBoolean("amoled", true),
            dynamicColor = o.optBoolean("dynamicColor", false),
        )
    } catch (_: Exception) {
        null
    }
}
