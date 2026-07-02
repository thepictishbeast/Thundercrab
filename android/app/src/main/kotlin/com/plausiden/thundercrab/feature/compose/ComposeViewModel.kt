// ============================================================================
// feature/compose/ComposeViewModel.kt  (VIEWMODEL group)
// Sends an outbound message via the Repository. The SMTP password is supplied
// per-send by the UI (a dialog) and never retained (spec §5.2). The signature
// (from AppPrefs) is exposed so the screen can pre-fill it. AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.compose

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.plausiden.thundercrab.data.AppPrefs
import com.plausiden.thundercrab.data.ThunderCrabRepository
import com.plausiden.thundercrab.data.model.OutboundAttachment
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class ComposeViewModel(
    private val repo: ThunderCrabRepository,
    prefs: AppPrefs,
) : ViewModel() {

    /** Signature to pre-fill into the body (may be blank). */
    val signature: String = prefs.signature

    private val _sent = MutableStateFlow(false)
    val sent: StateFlow<Boolean> = _sent.asStateFlow()

    private val _sending = MutableStateFlow(false)
    val sending: StateFlow<Boolean> = _sending.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    /** Files staged for this message. The screen resolves picked Uris into
     *  owned [OutboundAttachment]s (bytes read off the content provider) and
     *  adds them here; the VM stays free of Android platform types. */
    private val _attachments = MutableStateFlow<List<OutboundAttachment>>(emptyList())
    val attachments: StateFlow<List<OutboundAttachment>> = _attachments.asStateFlow()

    fun addAttachment(attachment: OutboundAttachment) {
        _attachments.value = _attachments.value + attachment
    }

    fun removeAttachment(index: Int) {
        _attachments.value = _attachments.value.filterIndexed { i, _ -> i != index }
    }

    fun send(
        password: String,
        to: String,
        cc: String,
        subject: String,
        body: String,
        readReceipt: Boolean = false,
    ) {
        _sending.value = true
        viewModelScope.launch {
            repo.sendMessage(
                password = password,
                to = splitAddrs(to),
                cc = splitAddrs(cc),
                subject = subject,
                body = body,
                readReceipt = readReceipt,
                attachments = _attachments.value,
            ).fold(
                onSuccess = { _sent.value = true },
                onFailure = { e ->
                    _sending.value = false
                    _error.value = e.message ?: "Couldn't send message."
                },
            )
        }
    }

    fun consumeError() {
        _error.value = null
    }

    private fun splitAddrs(raw: String): List<String> =
        raw.split(',', ';', '\n', ' ').map { it.trim() }.filter { it.isNotBlank() }

    companion object {
        fun factory(repo: ThunderCrabRepository, prefs: AppPrefs): ViewModelProvider.Factory =
            viewModelFactory {
                initializer { ComposeViewModel(repo, prefs) }
            }
    }
}
