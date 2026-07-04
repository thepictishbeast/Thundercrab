// ============================================================================
// feature/read/MessageReadScreen.kt  (UI group)
// Screen 4 — Message Read. Shows from / subject / all other headers, read from
// the VM's cached header (MessageReadUiState). There is NO body field on the
// UiState; the body region is the fixed DisabledBodyCard.
//
// Governance (AVP-2, no-body invariant, spec §5.1): this screen never requests
// a message body. On open it may mark the message seen via the VM. AVP-2:
// UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.read

import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.plausiden.thundercrab.data.model.Attachment
import com.plausiden.thundercrab.data.model.ComposeDraft
import com.plausiden.thundercrab.ui.components.MessageBodyHtml
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MessageReadScreen(
    viewModel: MessageReadViewModel,
    onBack: () -> Unit,
    onOpenCompose: (ComposeDraft) -> Unit,
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val draftReady by viewModel.draftReady.collectAsStateWithLifecycle()
    val preparingForward by viewModel.preparingForward.collectAsStateWithLifecycle()

    // Mark seen once on open (best-effort; VM owns the FFI call).
    LaunchedEffect(Unit) {
        viewModel.markSeen()
    }

    // When Reply/Forward has built a draft, hand it to the compose flow.
    LaunchedEffect(draftReady) {
        draftReady?.let { draft ->
            onOpenCompose(draft)
            viewModel.consumeDraft()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Message") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                        )
                    }
                },
                actions = {
                    if (state.found) {
                        TextButton(onClick = { viewModel.reply() }) { Text("Reply") }
                        if (preparingForward) {
                            CircularProgressIndicator(
                                modifier = Modifier.padding(horizontal = 12.dp).height(20.dp),
                                strokeWidth = 2.dp,
                            )
                        } else {
                            TextButton(onClick = { viewModel.forward() }) { Text("Forward") }
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant,
                ),
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (!state.found) {
                Text(
                    text = "Message header not available.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                return@Column
            }

            Text(
                text = state.subject.ifBlank { "(no subject)" },
                style = MaterialTheme.typography.headlineMedium,
            )
            Text(
                text = state.from.ifBlank { "(unknown sender)" },
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            state.readReceiptRequested?.let { ReadReceiptNotice(it) }

            HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)

            if (state.headers.isNotEmpty()) {
                Text(
                    text = "Headers",
                    style = MaterialTheme.typography.titleMedium,
                )
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    state.headers.forEach { (name, value) ->
                        HeaderRow(name = name, value = value)
                    }
                }
                HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)
            }

            Spacer(modifier = Modifier.height(4.dp))

            MessageBodySection(state)

            if (state.attachments.isNotEmpty()) {
                AttachmentsSection(
                    attachments = state.attachments,
                    loadBytes = viewModel::loadAttachmentBytes,
                )
            }
        }
    }
}

@Composable
private fun AttachmentsSection(
    attachments: List<Attachment>,
    loadBytes: suspend (index: Int) -> Result<ByteArray>,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    // Which attachment the user is currently saving (set just before launching
    // the file picker, consumed in its result callback).
    var pendingIndex by remember { mutableStateOf<Int?>(null) }

    // Storage Access Framework: the user picks the destination, so the app needs
    // no storage permission and writes only where the user explicitly chose.
    val createDocument = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/octet-stream"),
    ) { uri ->
        val index = pendingIndex
        pendingIndex = null
        if (uri == null || index == null) return@rememberLauncherForActivityResult
        scope.launch {
            loadBytes(index)
                .onSuccess { bytes ->
                    val ok = runCatching {
                        context.contentResolver.openOutputStream(uri)?.use { it.write(bytes) }
                    }.isSuccess
                    Toast.makeText(
                        context,
                        if (ok) "Attachment saved" else "Couldn't write the file",
                        Toast.LENGTH_SHORT,
                    ).show()
                }
                .onFailure { e ->
                    Toast.makeText(
                        context,
                        "Couldn't fetch attachment: ${e.message}",
                        Toast.LENGTH_LONG,
                    ).show()
                }
        }
    }

    HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)
    Text(
        text = "Attachments (${attachments.size})",
        style = MaterialTheme.typography.titleMedium,
    )
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        attachments.forEachIndexed { index, att ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = att.filename.ifBlank { "(unnamed)" },
                        style = MaterialTheme.typography.bodyMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = "${att.mimeType} · ${humanSize(att.size)}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                TextButton(onClick = {
                    pendingIndex = index
                    createDocument.launch(att.filename.ifBlank { "attachment_$index" })
                }) {
                    Text("Save")
                }
            }
        }
    }
}

/** Compact human-readable byte size (e.g. 12.3 KB). */
private fun humanSize(bytes: Long): String {
    if (bytes < 1024) return "$bytes B"
    val units = listOf("KB", "MB", "GB")
    var value = bytes.toDouble() / 1024
    var unit = 0
    while (value >= 1024 && unit < units.lastIndex) {
        value /= 1024
        unit++
    }
    return String.format(java.util.Locale.US, "%.1f %s", value, units[unit])
}

@Composable
private fun MessageBodySection(state: MessageReadUiState) {
    when {
        state.bodyLoading -> {
            Row(verticalAlignment = Alignment.CenterVertically) {
                CircularProgressIndicator(modifier = Modifier.height(18.dp).width(18.dp))
                Spacer(modifier = Modifier.width(10.dp))
                Text(
                    text = "Loading message…",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        state.bodyError != null -> {
            Text(
                text = "Couldn't load the message body: ${state.bodyError}",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
            )
        }

        // Prefer the sanitized HTML rendering when present.
        !state.bodyHtml.isNullOrBlank() -> {
            MessageBodyHtml(
                html = state.bodyHtml,
                modifier = Modifier.fillMaxWidth().height(420.dp),
            )
        }

        !state.bodyPlain.isNullOrBlank() -> {
            Text(
                text = state.bodyPlain,
                style = MaterialTheme.typography.bodyMedium,
            )
        }

        else -> {
            Text(
                text = "(no message body)",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun ReadReceiptNotice(address: String) {
    Surface(
        color = MaterialTheme.colorScheme.tertiaryContainer,
        shape = MaterialTheme.shapes.small,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(
            text = "The sender requested a read receipt ($address). ThunderCrab will not send one automatically.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onTertiaryContainer,
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
        )
    }
}

@Composable
private fun HeaderRow(name: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.Top,
    ) {
        Text(
            text = name,
            style = MaterialTheme.typography.labelMedium,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.width(120.dp),
        )
        Spacer(modifier = Modifier.width(8.dp))
        Text(
            text = value,
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.weight(1f),
        )
    }
}
