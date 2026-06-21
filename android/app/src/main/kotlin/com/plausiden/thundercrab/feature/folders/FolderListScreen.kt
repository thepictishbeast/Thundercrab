// ============================================================================
// feature/folders/FolderListScreen.kt  (UI group)
// Mailbox list + management. Reached from the inbox via the Menu icon.
// Thunderbird-style: branded header, rows with a special-use icon, name, count,
// and an unread badge. A New-folder FAB creates mailboxes; a per-row overflow
// renames/deletes them (IMAP CREATE/RENAME/DELETE via the Repository). INBOX and
// special-use folders are protected from rename/delete. AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.folders

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Badge
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.plausiden.thundercrab.data.model.ErrorKind
import com.plausiden.thundercrab.data.model.Folder

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FolderListScreen(
    viewModel: FolderListViewModel,
    onFolderClick: (String) -> Unit,
    onOpenSuggestions: () -> Unit,
    onBack: () -> Unit,
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val action by viewModel.action.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }

    var showCreate by remember { mutableStateOf(false) }
    var renameTarget by remember { mutableStateOf<Folder?>(null) }
    var deleteTarget by remember { mutableStateOf<Folder?>(null) }

    LaunchedEffect(action) {
        action?.let {
            snackbarHostState.showSnackbar(it)
            viewModel.consumeAction()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Mailboxes", fontWeight = FontWeight.SemiBold) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back to inbox")
                    }
                },
                actions = {
                    IconButton(onClick = onOpenSuggestions) {
                        Icon(Icons.Filled.Settings, contentDescription = "Rule suggestions")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface,
                    titleContentColor = MaterialTheme.colorScheme.onSurface,
                    navigationIconContentColor = MaterialTheme.colorScheme.primary,
                ),
            )
        },
        floatingActionButton = {
            FloatingActionButton(onClick = { showCreate = true }) {
                Icon(Icons.Filled.Add, contentDescription = "New mailbox")
            }
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
        ) {
            when (val s = state) {
                is FolderListUiState.Loading ->
                    CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))

                is FolderListUiState.Error -> ErrorState(
                    kind = s.kind,
                    message = s.message,
                    onRetry = viewModel::refresh,
                    modifier = Modifier.align(Alignment.Center),
                )

                is FolderListUiState.Success -> {
                    if (s.folders.isEmpty()) {
                        Text(
                            text = "No mailboxes found.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.align(Alignment.Center),
                        )
                    } else {
                        LazyColumn(modifier = Modifier.fillMaxSize()) {
                            item { MailboxesHeader(count = s.folders.size, unread = s.folders.sumOf { it.unseen }) }
                            items(items = s.folders, key = { it.name }) { folder ->
                                FolderRow(
                                    folder = folder,
                                    onClick = { onFolderClick(folder.name) },
                                    onRename = { renameTarget = folder },
                                    onDelete = { deleteTarget = folder },
                                )
                                HorizontalDivider(
                                    color = MaterialTheme.colorScheme.outlineVariant,
                                    thickness = 0.5.dp,
                                    modifier = Modifier.padding(start = 64.dp),
                                )
                            }
                        }
                    }
                }
            }
        }
    }

    if (showCreate) {
        FolderNameDialog(
            title = "New mailbox",
            label = "Mailbox name",
            initial = "",
            confirmText = "Create",
            onDismiss = { showCreate = false },
            onConfirm = { name -> showCreate = false; viewModel.createFolder(name) },
        )
    }
    renameTarget?.let { f ->
        FolderNameDialog(
            title = "Rename mailbox",
            label = "New name",
            initial = f.name,
            confirmText = "Rename",
            onDismiss = { renameTarget = null },
            onConfirm = { newName -> renameTarget = null; viewModel.renameFolder(f.name, newName) },
        )
    }
    deleteTarget?.let { f ->
        AlertDialog(
            onDismissRequest = { deleteTarget = null },
            title = { Text("Delete mailbox?") },
            text = {
                Text(
                    "Delete \"${f.name}\"" +
                        if (f.messages > 0) " and its ${f.messages} message(s)? This cannot be undone." else "?",
                )
            },
            confirmButton = {
                TextButton(onClick = { deleteTarget = null; viewModel.deleteFolder(f.name) }) {
                    Text("Delete", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = { TextButton(onClick = { deleteTarget = null }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun MailboxesHeader(count: Int, unread: Long) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.primaryContainer)
            .padding(horizontal = 20.dp, vertical = 18.dp),
    ) {
        Column {
            Text(
                text = "ThunderCrab",
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onPrimaryContainer,
            )
            Text(
                text = if (unread > 0) "$count mailboxes · $unread unread" else "$count mailboxes",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onPrimaryContainer,
            )
        }
    }
}

@Composable
private fun FolderRow(
    folder: Folder,
    onClick: () -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
) {
    var menuExpanded by remember { mutableStateOf(false) }
    val protected = isProtected(folder)

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(start = 20.dp, end = 4.dp, top = 14.dp, bottom = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Icon(
            imageVector = folderIcon(folder),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.primary,
        )
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = folder.name,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = if (folder.unseen > 0) FontWeight.SemiBold else FontWeight.Normal,
            )
            Text(
                text = "${folder.messages} total",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (folder.unseen > 0) {
            Badge(
                containerColor = MaterialTheme.colorScheme.primary,
                contentColor = MaterialTheme.colorScheme.onPrimary,
            ) { Text(folder.unseen.toString()) }
        }
        // Management overflow — hidden for protected (INBOX / special-use) folders.
        if (!protected) {
            Box {
                IconButton(onClick = { menuExpanded = true }) {
                    Icon(
                        Icons.Filled.MoreVert,
                        contentDescription = "Mailbox actions",
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                DropdownMenu(expanded = menuExpanded, onDismissRequest = { menuExpanded = false }) {
                    DropdownMenuItem(text = { Text("Rename") }, onClick = { menuExpanded = false; onRename() })
                    DropdownMenuItem(text = { Text("Delete") }, onClick = { menuExpanded = false; onDelete() })
                }
            }
        }
    }
}

@Composable
private fun FolderNameDialog(
    title: String,
    label: String,
    initial: String,
    confirmText: String,
    onDismiss: () -> Unit,
    onConfirm: (String) -> Unit,
) {
    var name by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text(label) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(
                onClick = { onConfirm(name.trim()) },
                enabled = name.isNotBlank() && name.trim() != initial,
            ) { Text(confirmText) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

/** INBOX and special-use mailboxes are protected from rename/delete. */
private fun isProtected(folder: Folder): Boolean =
    folder.name.equals("INBOX", ignoreCase = true) || !folder.specialUse.isNullOrBlank()

/** Map a folder to a core Material icon by special-use marker / name. */
private fun folderIcon(folder: Folder): ImageVector {
    val su = folder.specialUse?.lowercase().orEmpty()
    return when {
        folder.name.equals("INBOX", ignoreCase = true) || su.contains("inbox") -> Icons.Filled.Email
        su.contains("sent") -> Icons.AutoMirrored.Filled.Send
        su.contains("draft") -> Icons.Filled.Edit
        su.contains("trash") -> Icons.Filled.Delete
        su.contains("junk") -> Icons.Filled.Warning
        else -> Icons.AutoMirrored.Filled.List
    }
}

@Composable
private fun ErrorState(kind: ErrorKind, message: String, onRetry: () -> Unit, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("Couldn't load mailboxes", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.error)
        Text(
            text = "${kind.name}: $message",
            style = MaterialTheme.typography.bodySmall,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Button(onClick = onRetry) { Text("Retry") }
    }
}
