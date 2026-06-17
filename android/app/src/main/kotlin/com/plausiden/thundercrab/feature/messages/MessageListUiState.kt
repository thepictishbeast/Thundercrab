// ============================================================================
// feature/messages/MessageListUiState.kt  (VIEWMODEL group)
// Header-only message rows. Uses DATA domain model MessageHeader + ErrorKind;
// no Ffi* type and no body field anywhere. Spec §4 / §5.1 / VIEWMODEL contracts.
// ============================================================================
package com.plausiden.thundercrab.feature.messages

import com.plausiden.thundercrab.data.model.ErrorKind
import com.plausiden.thundercrab.data.model.MessageHeader

data class MessageListUiState(
    val folder: String,
    val loading: Boolean = true,
    val messages: List<MessageHeader> = emptyList(),
    val errorKind: ErrorKind? = null,
    val errorMessage: String? = null,
)
