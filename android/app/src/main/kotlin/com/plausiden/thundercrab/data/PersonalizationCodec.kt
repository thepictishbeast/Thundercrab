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

    /**
     * Serialize the appearance + signature personalization blob. (Sorting rules
     * sync via their own "rules" METADATA entry so they can push the instant they
     * change, independent of this blob.)
     */
    fun encode(appearance: AppearancePrefs, signature: String): String =
        JSONObject()
            .put("v", VERSION)
            .put("themeMode", appearance.themeMode.name)
            .put("amoled", appearance.amoled)
            .put("dynamicColor", appearance.dynamicColor)
            .put("signature", signature)
            .toString()

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
