// ============================================================================
// feature/read/MessageReadUiState.kt  (VIEWMODEL group)
// Read state: cached header PLUS the fetched message body (display-only — the
// body never feeds the rules/suggestions spine). The body's HTML is already
// sanitized by the core and safe for a locked-down WebView.
// ============================================================================
package com.plausiden.thundercrab.feature.read

import com.plausiden.thundercrab.data.model.Attachment

data class MessageReadUiState(
    val from: String = "",
    val subject: String = "",
    val headers: List<Pair<String, String>> = emptyList(),
    val found: Boolean = false,
    /** If the sender requested a read receipt, the address; null otherwise. */
    val readReceiptRequested: String? = null,
    // --- Body display ---
    val bodyLoading: Boolean = false,
    /** Sanitized HTML to render in the WebView (preferred when present). */
    val bodyHtml: String? = null,
    /** Plain-text fallback (shown when there is no HTML part). */
    val bodyPlain: String? = null,
    /** Non-null if the body fetch failed. */
    val bodyError: String? = null,
    /** Attachments on the message (metadata only; bytes fetched on save). */
    val attachments: List<Attachment> = emptyList(),
)
