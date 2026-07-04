// ============================================================================
// feature/compose/ComposeScreen.kt  (UI group)
// Compose + send an outbound message. Signature (from settings) is pre-filled.
// On Send, a dialog collects the SMTP password (never retained — spec §5.2) and
// the message goes out via the Repository → SMTP. Outbound only; the no-body
// READ invariant is unaffected. AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.compose

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.InputChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Switch
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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.plausiden.thundercrab.data.model.OutboundAttachment
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun ComposeScreen(
    viewModel: ComposeViewModel,
    onDone: () -> Unit,
) {
    val sent by viewModel.sent.collectAsStateWithLifecycle()
    val sending by viewModel.sending.collectAsStateWithLifecycle()
    val error by viewModel.error.collectAsStateWithLifecycle()
    val attachments by viewModel.attachments.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    // SAF file picker. OpenDocument grants read on the returned Uri; we read the
    // bytes immediately (off the content provider) into an owned attachment so no
    // long-lived Uri permission is needed. Oversized files are rejected here.
    val pickFile = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri: Uri? ->
        if (uri == null) return@rememberLauncherForActivityResult
        val attachment = readAttachment(context, uri)
        when {
            attachment == null ->
                scope.launch { snackbar.showSnackbar("Couldn't read that file.") }
            attachment.bytes.size > MAX_ATTACHMENT_BYTES ->
                scope.launch { snackbar.showSnackbar("That file is too large (max 25 MB).") }
            else -> viewModel.addAttachment(attachment)
        }
    }

    // Seed from the ViewModel's initial values (prefilled for reply/forward,
    // blank + signature for a fresh compose). rememberSaveable so edits survive
    // rotation AND process death — the initializer still runs only on first
    // composition. The reply threading IDs ride along so a restored reply still
    // threads even after the one-shot draft holder is gone.
    var to by rememberSaveable { mutableStateOf(viewModel.initialTo) }
    var cc by rememberSaveable { mutableStateOf(viewModel.initialCc) }
    var subject by rememberSaveable { mutableStateOf(viewModel.initialSubject) }
    var body by rememberSaveable { mutableStateOf(viewModel.initialBody) }
    val inReplyTo by rememberSaveable { mutableStateOf(viewModel.initialInReplyTo) }
    val references by rememberSaveable { mutableStateOf(viewModel.initialReferences) }
    var requestReceipt by rememberSaveable { mutableStateOf(false) }
    // Transient: the "enter password" dialog flag never persists, and the
    // password field itself (below) is deliberately non-saveable (§5.2).
    var askPassword by remember { mutableStateOf(false) }

    LaunchedEffect(sent) { if (sent) onDone() }
    LaunchedEffect(error) { error?.let { snackbar.showSnackbar(it); viewModel.consumeError() } }

    val canSend = to.isNotBlank() && subject.isNotBlank() && !sending

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("New message", fontWeight = FontWeight.SemiBold) },
                navigationIcon = {
                    IconButton(onClick = onDone) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Discard")
                    }
                },
                actions = {
                    if (sending) {
                        CircularProgressIndicator(modifier = Modifier.padding(end = 16.dp), strokeWidth = 2.dp)
                    } else {
                        IconButton(onClick = { askPassword = true }, enabled = canSend) {
                            Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Send")
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface,
                    titleContentColor = MaterialTheme.colorScheme.onSurface,
                    navigationIconContentColor = MaterialTheme.colorScheme.primary,
                    actionIconContentColor = MaterialTheme.colorScheme.primary,
                ),
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { pad ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(pad)
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            OutlinedTextField(value = to, onValueChange = { to = it }, label = { Text("To") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(value = cc, onValueChange = { cc = it }, label = { Text("Cc (optional)") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(value = subject, onValueChange = { subject = it }, label = { Text("Subject") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(
                value = body,
                onValueChange = { body = it },
                label = { Text("Message") },
                modifier = Modifier.fillMaxWidth(),
                minLines = 8,
            )

            // Attachments: pick a file, then show a removable chip per staged file.
            TextButton(onClick = { pickFile.launch(arrayOf("*/*")) }) {
                Icon(Icons.Filled.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("Attach file")
            }
            if (attachments.isNotEmpty()) {
                FlowRow(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    attachments.forEachIndexed { index, att ->
                        InputChip(
                            selected = false,
                            onClick = { },
                            label = { Text(att.filename, maxLines = 1) },
                            trailingIcon = {
                                Icon(
                                    Icons.Filled.Close,
                                    contentDescription = "Remove ${att.filename}",
                                    modifier = Modifier
                                        .size(18.dp)
                                        .clickable { viewModel.removeAttachment(index) },
                                )
                            },
                        )
                    }
                }
            }

            Row(
                modifier = Modifier.fillMaxWidth().padding(bottom = 24.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = "Request read receipt",
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.weight(1f),
                )
                Switch(checked = requestReceipt, onCheckedChange = { requestReceipt = it })
            }
        }
    }

    if (askPassword) {
        var pw by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { askPassword = false },
            title = { Text("Send message") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Enter your mail password to send. It is used once and never stored.", style = MaterialTheme.typography.bodyMedium)
                    OutlinedTextField(
                        value = pw,
                        onValueChange = { pw = it },
                        label = { Text("Password") },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        askPassword = false
                        viewModel.send(pw, to, cc, subject, body, requestReceipt, inReplyTo, references)
                    },
                    enabled = pw.isNotBlank(),
                ) { Text("Send") }
            },
            dismissButton = { TextButton(onClick = { askPassword = false }) { Text("Cancel") } },
        )
    }
}

/** Cap on how large a picked file we'll read into memory (25 MB). Guards against
 *  OOM on a huge selection; most mail servers reject larger messages anyway. */
private const val MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024

/**
 * Resolve a picked `content://` [uri] into an owned [OutboundAttachment] by
 * reading its bytes through the [context] ContentResolver. Returns null if the
 * stream can't be opened. The display name comes from `OpenableColumns`, falling
 * back to the Uri's last path segment; the MIME type from `getType`.
 */
private fun readAttachment(context: Context, uri: Uri): OutboundAttachment? {
    val resolver = context.contentResolver
    val name = resolver
        .query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { cursor -> if (cursor.moveToFirst() && !cursor.isNull(0)) cursor.getString(0) else null }
        ?: uri.lastPathSegment
        ?: "attachment"
    val mime = resolver.getType(uri) ?: "application/octet-stream"
    val bytes = resolver.openInputStream(uri)?.use { it.readBytes() } ?: return null
    return OutboundAttachment(filename = name, mimeType = mime, bytes = bytes)
}
