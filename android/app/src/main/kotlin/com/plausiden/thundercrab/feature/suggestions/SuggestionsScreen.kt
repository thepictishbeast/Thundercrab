// ============================================================================
// feature/suggestions/SuggestionsScreen.kt  (UI group)
// Suggestions — previews derived sorting rules, accepts (saves) them, lists the
// saved rules (with delete), and shows the emitted Sieve script READ-ONLY in a
// monospace card. Renders SuggestionsUiState; no Ffi* type appears here.
//
// AVP-2 GUARDRAIL: the "Push to server" affordance is a VISIBLY DISABLED
// placeholder. There is no real push and no Repository push method to call.
// AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.suggestions

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.plausiden.thundercrab.data.model.RuleSuggestion

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SuggestionsScreen(
    viewModel: SuggestionsViewModel,
    onBack: () -> Unit,
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Suggestions") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant,
                ),
            )
        },
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
        ) {
            if (state.loading) {
                CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
            } else {
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = androidx.compose.foundation.layout.PaddingValues(20.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    state.errorMessage?.let { message ->
                        item("error") {
                            ErrorBanner(
                                text = "${state.errorKind?.name ?: "ERROR"}: $message",
                                onDismiss = viewModel::consumeError,
                            )
                        }
                    }

                    // --- Suggested rules ------------------------------------
                    item("suggestions-header") {
                        SectionHeader("Suggested rules")
                    }
                    if (state.suggestions.isEmpty()) {
                        item("suggestions-empty") {
                            EmptyHint("No new suggestions. Sort more mail to derive rules.")
                        }
                    } else {
                        items(items = state.suggestions, key = { "sg-${it.id}" }) { rule ->
                            RuleCard(
                                rule = rule,
                                busy = rule.id in state.busyIds,
                                trailing = {
                                    Button(
                                        onClick = { viewModel.accept(rule.id) },
                                        enabled = rule.id !in state.busyIds,
                                    ) { Text("Accept") }
                                },
                            )
                        }
                    }

                    // --- Saved rules ----------------------------------------
                    item("saved-header") {
                        SectionHeader("Saved rules")
                    }
                    if (state.savedRules.isEmpty()) {
                        item("saved-empty") {
                            EmptyHint("No saved rules yet. Accept a suggestion to add one.")
                        }
                    } else {
                        items(items = state.savedRules, key = { "sv-${it.id}" }) { rule ->
                            RuleCard(
                                rule = rule,
                                busy = rule.id in state.busyIds,
                                trailing = {
                                    IconButton(
                                        onClick = { viewModel.delete(rule.id) },
                                        enabled = rule.id !in state.busyIds,
                                    ) {
                                        Icon(
                                            imageVector = Icons.Filled.Delete,
                                            contentDescription = "Delete rule",
                                            tint = MaterialTheme.colorScheme.error,
                                        )
                                    }
                                },
                            )
                        }
                    }

                    // --- Emitted Sieve (read-only) --------------------------
                    item("sieve-header") {
                        SectionHeader("Emitted Sieve (read-only)")
                    }
                    item("sieve-card") {
                        SieveCard(sieve = state.sieve)
                    }
                    item("push-affordance") {
                        // AVP-2 GUARDRAIL: disabled placeholder; no real push.
                        OutlinedButton(
                            onClick = {},
                            enabled = false,
                            modifier = Modifier.fillMaxWidth(),
                        ) { Text("Push to server — not enabled yet") }
                    }
                }
            }
        }
    }
}

@Composable
private fun SectionHeader(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.onSurface,
    )
}

@Composable
private fun EmptyHint(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun RuleCard(
    rule: RuleSuggestion,
    busy: Boolean,
    trailing: @Composable () -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant,
        ),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = rule.displayName,
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    text = rule.summary,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (busy) {
                CircularProgressIndicator(modifier = Modifier.padding(end = 8.dp))
            } else {
                trailing()
            }
        }
    }
}

@Composable
private fun SieveCard(sieve: String) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surface,
        ),
    ) {
        Box(modifier = Modifier.horizontalScroll(rememberScrollState())) {
            Text(
                text = sieve.ifBlank { "# No rules — empty script." },
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.padding(16.dp),
            )
        }
    }
}

@Composable
private fun ErrorBanner(text: String, onDismiss: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.errorContainer,
        ),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(
                text = text,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onErrorContainer,
            )
            HorizontalDivider(
                modifier = Modifier.padding(vertical = 8.dp),
                color = MaterialTheme.colorScheme.onErrorContainer,
            )
            OutlinedButton(onClick = onDismiss) { Text("Dismiss") }
        }
    }
}
