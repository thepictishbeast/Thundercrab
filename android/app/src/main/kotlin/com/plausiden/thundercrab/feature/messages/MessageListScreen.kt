// ============================================================================
// feature/messages/MessageListScreen.kt  (UI group)
// Message List — the app HOME (opens on INBOX). Thunderbird-style rows: a
// colored sender avatar, sender, and subject (HEADER-ONLY; no body preview, by
// the no-body invariant). Tap a row → read. The nav icon is a Menu that opens
// the mailbox list. Per-row overflow offers flag/seen/move; each calls the VM,
// which is where the Repository emits a features-only FlagEvent internally.
// AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.messages

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.plausiden.thundercrab.data.model.MessageHeader
import com.plausiden.thundercrab.ui.components.SenderAvatar

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MessageListScreen(
    viewModel: MessageListViewModel,
    onOpenFolders: () -> Unit,
    onMessageClick: (Int) -> Unit,
    onCompose: () -> Unit,
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    var moveTargetUid by remember { mutableStateOf<Int?>(null) }
    var searchActive by remember { mutableStateOf(false) }
    var queryText by remember { mutableStateOf("") }

    LaunchedEffect(state.errorMessage) {
        val msg = state.errorMessage
        if (msg != null) {
            val kind = state.errorKind?.name ?: "ERROR"
            snackbarHostState.showSnackbar("$kind: $msg")
            viewModel.consumeError()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = prettyFolder(state.folder),
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onOpenFolders) {
                        Icon(
                            imageVector = Icons.Filled.Menu,
                            contentDescription = "Mailboxes",
                        )
                    }
                },
                actions = {
                    IconButton(onClick = {
                        searchActive = !searchActive
                        if (!searchActive) {
                            queryText = ""
                            if (state.isSearchResult) viewModel.clearSearch()
                        }
                    }) {
                        Icon(
                            imageVector = if (searchActive) Icons.Filled.Close else Icons.Filled.Search,
                            contentDescription = if (searchActive) "Close search" else "Search mailbox",
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface,
                    titleContentColor = MaterialTheme.colorScheme.onSurface,
                    navigationIconContentColor = MaterialTheme.colorScheme.primary,
                ),
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
        floatingActionButton = {
            FloatingActionButton(onClick = onCompose) {
                Icon(Icons.Filled.Edit, contentDescription = "Compose")
            }
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
        ) {
            if (searchActive) {
                SearchField(
                    value = queryText,
                    onValueChange = { queryText = it },
                    onSubmit = { viewModel.onSearch(queryText) },
                    onClear = {
                        queryText = ""
                        if (state.isSearchResult) viewModel.clearSearch()
                    },
                )
            }
            if (state.isSearchResult && !state.loading) {
                Text(
                    text = "${state.messages.size} result(s) for “${state.query}”",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp),
                )
            }
            Box(modifier = Modifier.fillMaxSize()) {
                when {
                    state.loading -> CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))

                    state.messages.isEmpty() -> EmptyOrError(
                        errorMessage = state.errorMessage,
                        isSearchResult = state.isSearchResult,
                        onRetry = viewModel::refresh,
                        modifier = Modifier.align(Alignment.Center),
                    )

                    else -> LazyColumn(modifier = Modifier.fillMaxSize()) {
                        items(items = state.messages, key = { it.uid }) { header ->
                            MessageRow(
                                header = header,
                                onClick = { onMessageClick(header.uid) },
                                onMarkSeen = { viewModel.onToggleSeen(header.uid, true) },
                                onMarkUnseen = { viewModel.onToggleSeen(header.uid, false) },
                                onFlag = { viewModel.onToggleFlagged(header.uid, true) },
                                onUnflag = { viewModel.onToggleFlagged(header.uid, false) },
                                onMove = { moveTargetUid = header.uid },
                            )
                            HorizontalDivider(
                                color = MaterialTheme.colorScheme.outlineVariant,
                                thickness = 0.5.dp,
                                modifier = Modifier.padding(start = 74.dp),
                            )
                        }
                    }
                }
            }
        }
    }

    val targetUid = moveTargetUid
    if (targetUid != null) {
        MoveDialog(
            onDismiss = { moveTargetUid = null },
            onConfirm = { destination ->
                viewModel.onMove(targetUid, destination)
                moveTargetUid = null
            },
        )
    }
}

/** Strip the IMAP hierarchy prefix for a friendlier title (Archive/2026 → 2026). */
private fun prettyFolder(raw: String): String =
    raw.substringAfterLast('/').ifBlank { raw }

@Composable
private fun MessageRow(
    header: MessageHeader,
    onClick: () -> Unit,
    onMarkSeen: () -> Unit,
    onMarkUnseen: () -> Unit,
    onFlag: () -> Unit,
    onUnflag: () -> Unit,
    onMove: () -> Unit,
) {
    var menuExpanded by remember { mutableStateOf(false) }
    val sender = header.from.ifBlank { "(unknown sender)" }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(start = 16.dp, end = 4.dp, top = 12.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        SenderAvatar(seed = sender)
        Spacer(Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = sender,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Medium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = header.subject.ifBlank { "(no subject)" },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Box {
            IconButton(onClick = { menuExpanded = true }) {
                Icon(
                    imageVector = Icons.Filled.MoreVert,
                    contentDescription = "Message actions",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            DropdownMenu(expanded = menuExpanded, onDismissRequest = { menuExpanded = false }) {
                DropdownMenuItem(text = { Text("Mark as read") }, onClick = { menuExpanded = false; onMarkSeen() })
                DropdownMenuItem(text = { Text("Mark as unread") }, onClick = { menuExpanded = false; onMarkUnseen() })
                DropdownMenuItem(text = { Text("Flag") }, onClick = { menuExpanded = false; onFlag() })
                DropdownMenuItem(text = { Text("Unflag") }, onClick = { menuExpanded = false; onUnflag() })
                DropdownMenuItem(text = { Text("Move to…") }, onClick = { menuExpanded = false; onMove() })
            }
        }
    }
}

@Composable
private fun MoveDialog(onDismiss: () -> Unit, onConfirm: (String) -> Unit) {
    var destination by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Move message") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Enter the destination mailbox name.", style = MaterialTheme.typography.bodyMedium)
                OutlinedTextField(
                    value = destination,
                    onValueChange = { destination = it },
                    label = { Text("Mailbox") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(destination.trim()) }, enabled = destination.isNotBlank()) {
                Text("Move")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SearchField(
    value: String,
    onValueChange: (String) -> Unit,
    onSubmit: () -> Unit,
    onClear: () -> Unit,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 8.dp),
        placeholder = { Text("Search this mailbox") },
        leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
        trailingIcon = {
            if (value.isNotEmpty()) {
                IconButton(onClick = onClear) {
                    Icon(Icons.Filled.Close, contentDescription = "Clear search")
                }
            }
        },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { onSubmit() }),
    )
}

@Composable
private fun EmptyOrError(
    errorMessage: String?,
    isSearchResult: Boolean,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier.padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        when {
            errorMessage != null -> {
                Text("Couldn't load messages", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.error)
                Text(
                    text = errorMessage,
                    style = MaterialTheme.typography.bodySmall,
                    textAlign = TextAlign.Center,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Button(onClick = onRetry) { Text("Refresh") }
            }
            isSearchResult -> {
                Text("No messages matched your search.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            else -> {
                Text("No messages in this mailbox.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Button(onClick = onRetry) { Text("Refresh") }
            }
        }
    }
}
