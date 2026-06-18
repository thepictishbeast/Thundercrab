// ============================================================================
// feature/folders/FolderListScreen.kt  (UI group)
// Mailbox list — reached from the inbox via the Menu icon. Thunderbird-style:
// a branded header, then rows with a special-use icon, name, count, and an
// unread badge. Tapping a row navigates to messages/{folder}. AVP-2: UNVERIFIED.
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
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Badge
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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
                                FolderRow(folder = folder, onClick = { onFolderClick(folder.name) })
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
private fun FolderRow(folder: Folder, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 16.dp),
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
    }
}

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
