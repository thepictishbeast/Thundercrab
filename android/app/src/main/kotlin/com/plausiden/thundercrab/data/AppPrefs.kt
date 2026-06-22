// ============================================================================
// data/AppPrefs.kt  (DATA group)
// On-device appearance + privacy preferences, persisted in SharedPreferences.
// Reactive via StateFlow so MainActivity can re-theme live when Settings change.
// No secrets, no Ffi* — pure local UI state. (Cross-device SYNC of these prefs
// is the per-user-profile roadmap item; this is the local-first store.)
// ============================================================================
package com.plausiden.thundercrab.data

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** Which color scheme to apply. SYSTEM follows the OS dark/light setting. */
enum class ThemeMode { SYSTEM, LIGHT, DARK }

/** The appearance knobs that drive ThundercrabTheme. */
data class AppearancePrefs(
    val themeMode: ThemeMode = ThemeMode.SYSTEM,
    val amoled: Boolean = true,           // AMOLED true-black dark default (doctrine)
    val dynamicColor: Boolean = false,    // Material You (Android 12+)
)

class AppPrefs(context: Context) {
    private val sp = context.getSharedPreferences("thundercrab_prefs", Context.MODE_PRIVATE)

    private val _appearance = MutableStateFlow(loadAppearance())
    val appearance: StateFlow<AppearancePrefs> = _appearance.asStateFlow()

    fun setThemeMode(mode: ThemeMode) {
        sp.edit().putString(KEY_THEME, mode.name).apply()
        _appearance.value = _appearance.value.copy(themeMode = mode)
    }

    fun setAmoled(on: Boolean) {
        sp.edit().putBoolean(KEY_AMOLED, on).apply()
        _appearance.value = _appearance.value.copy(amoled = on)
    }

    fun setDynamicColor(on: Boolean) {
        sp.edit().putBoolean(KEY_DYNAMIC, on).apply()
        _appearance.value = _appearance.value.copy(dynamicColor = on)
    }

    /** Whether the first-run diagnostics notice has been shown + answered. */
    var consentShown: Boolean
        get() = sp.getBoolean(KEY_CONSENT, false)
        set(v) = sp.edit().putBoolean(KEY_CONSENT, v).apply()

    /**
     * The user's diagnostics choice, persisted (the Rust master switch is
     * in-memory and resets to on each process start, so we re-apply this at
     * startup). Defaults on; the first-run notice + Settings let the user opt out.
     */
    var telemetryEnabled: Boolean
        get() = sp.getBoolean(KEY_TELEMETRY, true)
        set(v) = sp.edit().putBoolean(KEY_TELEMETRY, v).apply()

    /** Outgoing-mail signature, appended to composed messages. Not a secret. */
    var signature: String
        get() = sp.getString(KEY_SIGNATURE, "") ?: ""
        set(v) = sp.edit().putString(KEY_SIGNATURE, v).apply()

    private fun loadAppearance(): AppearancePrefs = AppearancePrefs(
        themeMode = runCatching { ThemeMode.valueOf(sp.getString(KEY_THEME, null) ?: "SYSTEM") }
            .getOrDefault(ThemeMode.SYSTEM),
        amoled = sp.getBoolean(KEY_AMOLED, true),
        dynamicColor = sp.getBoolean(KEY_DYNAMIC, false),
    )

    private companion object {
        const val KEY_THEME = "theme_mode"
        const val KEY_AMOLED = "amoled"
        const val KEY_DYNAMIC = "dynamic_color"
        const val KEY_CONSENT = "diag_consent_shown"
        const val KEY_TELEMETRY = "telemetry_enabled"
        const val KEY_SIGNATURE = "signature"
    }
}
