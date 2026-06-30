// ============================================================================
// feature/settings/SettingsViewModel.kt  (VIEWMODEL group)
// Drives the Settings screen: appearance prefs (via AppPrefs, persisted +
// reactive so the theme re-applies live) and on-device diagnostics/telemetry
// (via the Repository's PII-free FFI surface). No Ffi* type here. AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.plausiden.thundercrab.data.AppPrefs
import com.plausiden.thundercrab.data.PersonalizationCodec
import com.plausiden.thundercrab.data.AppearancePrefs
import com.plausiden.thundercrab.data.ThemeMode
import com.plausiden.thundercrab.data.ThunderCrabRepository
import com.plausiden.thundercrab.data.model.AccountDraft
import com.plausiden.thundercrab.data.model.DiagEvent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

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
        syncPersonalizationUp()
    }

    fun setThemeMode(mode: ThemeMode) { prefs.setThemeMode(mode); syncPersonalizationUp() }
    fun setAmoled(on: Boolean) { prefs.setAmoled(on); syncPersonalizationUp() }
    fun setDynamicColor(on: Boolean) { prefs.setDynamicColor(on); syncPersonalizationUp() }

    /**
     * Pull the synced personalization blob (if connected + the server has
     * METADATA) and apply it locally. Best-effort: any failure leaves local
     * prefs untouched (local-first). Call when the screen opens.
     */
    fun syncPersonalizationDown() {
        viewModelScope.launch {
            repo.getPersonalization().getOrNull()?.let { json ->
                PersonalizationCodec.decodeAppearance(json)?.let(prefs::applyAppearance)
                // Apply the synced signature directly (no re-push, to avoid a loop).
                PersonalizationCodec.decodeSignature(json)?.let { sig ->
                    prefs.signature = sig
                    _signature.value = sig
                }
            }
            // Sorting rules sync via their own entry (and push themselves on change).
            repo.syncRulesDown()
        }
    }

    /** Push the current appearance + signature to the user's mailbox (best-effort). */
    private fun syncPersonalizationUp() {
        viewModelScope.launch {
            repo.setPersonalization(
                PersonalizationCodec.encode(prefs.appearance.value, prefs.signature),
            )
        }
    }

    fun setTelemetry(on: Boolean) {
        repo.setTelemetryEnabled(on)
        prefs.telemetryEnabled = on   // persist so the choice survives restart
        _telemetry.value = on
        if (!on) {
            repo.clearDiagnostics()
            _diagnostics.value = emptyList()
        }
    }

    // --- Background IMAP IDLE push notifications ---

    private val _backgroundIdle = MutableStateFlow(prefs.backgroundIdle)
    val backgroundIdle: StateFlow<Boolean> = _backgroundIdle.asStateFlow()

    /** Host/ports for `email` from the core defaults (no network, no secret). */
    fun draftFor(email: String): AccountDraft = repo.accountDraftFor(email)

    /** Persist the account + flip the flag on. Credential storage + starting the
     *  service is the screen's job (it owns Context). */
    fun enableBackground(draft: AccountDraft) {
        prefs.saveAccount(draft)
        prefs.backgroundIdle = true
        _backgroundIdle.value = true
    }

    fun disableBackground() {
        prefs.backgroundIdle = false
        _backgroundIdle.value = false
    }

    /** Username of the persisted background account, if any (to clear its credential). */
    fun backgroundAccountUsername(): String? = prefs.loadAccount()?.username

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
