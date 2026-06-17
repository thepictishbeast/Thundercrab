// ============================================================================
// feature/read/MessageReadUiState.kt  (VIEWMODEL group)
// Header-only read state. NO body field by design — the body area is a fixed
// DISABLED UI state in the Screen ("Message body not available in this build").
// Never references fetchBody. Spec §5.1 / §2.1 / VIEWMODEL contracts.
// ============================================================================
package com.plausiden.thundercrab.feature.read

data class MessageReadUiState(
    val from: String = "",
    val subject: String = "",
    val headers: List<Pair<String, String>> = emptyList(),
    val found: Boolean = false,
    // NOTE: NO body field. Body is a fixed disabled UI state (spec §5.1).
)
