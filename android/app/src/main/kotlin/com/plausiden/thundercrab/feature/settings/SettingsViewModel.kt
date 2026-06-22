// ============================================================================
// feature/settings/SettingsViewModel.kt  (VIEWMODEL group)
// Drives the Settings screen: appearance prefs (via AppPrefs, persisted +
// reactive so the theme re-applies live) and on-device diagnostics/telemetry
// (via the Repository's PII-free FFI surface). No Ffi* type here. AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.plausiden.thundercrab.data.AppPrefs
import com.plausiden.thundercrab.data.AppearancePrefs
import com.plausiden.thundercrab.data.ThemeMode
import com.plausiden.thundercrab.data.ThunderCrabRepository
import com.plausiden.thundercrab.data.model.DiagEvent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

class SettingsViewModel(
    private val repo: ThunderCrabRepository,
    private val prefs: AppPrefs,
) : ViewModel() {

    val appearance: StateFlow<AppearancePrefs> = prefs.appearance

    private val _telemetry = MutableStateFlow(repo.telemetryEnabled())
    val telemetry: StateFlow<Boolean> = _telemetry.asStateFlow()

    private val _diagnostics = MutableStateFlow(repo.diagnostics())
    val diagnostics: StateFlow<List<DiagEvent>> = _diagnostics.asStateFlow()

    private val _signature = MutableStateFlow(prefs.signature)
    val signature: StateFlow<String> = _signature.asStateFlow()

    fun setSignature(s: String) {
        prefs.signature = s
        _signature.value = s
    }

    fun setThemeMode(mode: ThemeMode) = prefs.setThemeMode(mode)
    fun setAmoled(on: Boolean) = prefs.setAmoled(on)
    fun setDynamicColor(on: Boolean) = prefs.setDynamicColor(on)

    fun setTelemetry(on: Boolean) {
        repo.setTelemetryEnabled(on)
        prefs.telemetryEnabled = on   // persist so the choice survives restart
        _telemetry.value = on
        if (!on) {
            repo.clearDiagnostics()
            _diagnostics.value = emptyList()
        }
    }

    fun refreshDiagnostics() {
        _diagnostics.value = repo.diagnostics()
    }

    fun clearDiagnostics() {
        repo.clearDiagnostics()
        _diagnostics.value = emptyList()
    }

    companion object {
        fun factory(repo: ThunderCrabRepository, prefs: AppPrefs): ViewModelProvider.Factory =
            viewModelFactory {
                initializer { SettingsViewModel(repo, prefs) }
            }
    }
}
