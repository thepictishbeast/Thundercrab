// ============================================================================
// data/PersonalizationCodec.kt  (DATA group)
// Serializes the cross-device personalization blob synced via IMAP METADATA.
// Versioned + forward-compatible: unknown fields are ignored and missing ones
// fall back to defaults, so older and newer clients interoperate. Grow it by
// adding fields — never a protocol change. Uses framework org.json (no dep).
// ============================================================================
package com.plausiden.thundercrab.data

import org.json.JSONObject

object PersonalizationCodec {
    private const val VERSION = 1

    /** Serialize the appearance (and, later, more) into the synced blob. */
    fun encode(appearance: AppearancePrefs): String =
        JSONObject()
            .put("v", VERSION)
            .put("themeMode", appearance.themeMode.name)
            .put("amoled", appearance.amoled)
            .put("dynamicColor", appearance.dynamicColor)
            .toString()

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
