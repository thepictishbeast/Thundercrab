// ============================================================================
// feature/rules/RuleEditorScreen.kt  (UI group)
// Graphical mail-routing rule editor ("where mail goes"). The user picks a
// WHEN condition (from-domain / subject-contains / header-contains) and a DO
// action (move-to-folder / set-flag); the screen serializes these to canonical
// MatchExpr/Action JSON and saves via the Repository (origin = USER). Rules are
// emitted to a Sieve script (read-only preview elsewhere). AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.rules

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle

private enum class Cond(val label: String) { FROM_DOMAIN("From domain"), SUBJECT("Subject contains"), HEADER("Header contains") }
private enum class Act(val label: String) { MOVE("Move to folder"), FLAG("Set flag") }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RuleEditorScreen(
    viewModel: RuleEditorViewModel,
    onDone: () -> Unit,
) {
    val saved by viewModel.saved.collectAsStateWithLifecycle()
    val error by viewModel.error.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }

    var name by remember { mutableStateOf("") }
    var cond by remember { mutableStateOf(Cond.FROM_DOMAIN) }
    var v1 by remember { mutableStateOf("") }   // domain / subject text / header name
    var v2 by remember { mutableStateOf("") }   // header substring (HEADER only)
    var act by remember { mutableStateOf(Act.MOVE) }
    var av by remember { mutableStateOf("") }   // folder / flag

    LaunchedEffect(saved) { if (saved) onDone() }
    LaunchedEffect(error) {
        error?.let { snackbar.showSnackbar(it); viewModel.consumeError() }
    }

    val condValid = when (cond) {
        Cond.HEADER -> v1.isNotBlank() && v2.isNotBlank()
        else -> v1.isNotBlank()
    }
    val canSave = name.isNotBlank() && condValid && av.isNotBlank()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("New rule", fontWeight = FontWeight.SemiBold) },
                navigationIcon = {
                    IconButton(onClick = onDone) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Cancel")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface,
                    titleContentColor = MaterialTheme.colorScheme.onSurface,
                    navigationIconContentColor = MaterialTheme.colorScheme.primary,
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
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("Rule name") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )

            Text("WHEN a message…", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
            ChipRow(Cond.entries, cond) { cond = it }
            when (cond) {
                Cond.FROM_DOMAIN -> Field(v1, { v1 = it }, "Domain (e.g. github.com)")
                Cond.SUBJECT -> Field(v1, { v1 = it }, "Subject contains…")
                Cond.HEADER -> {
                    Field(v1, { v1 = it }, "Header name (e.g. List-Id)")
                    Field(v2, { v2 = it }, "Header contains…")
                }
            }

            Text("DO this:", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
            ChipRow(Act.entries, act) { act = it }
            when (act) {
                Act.MOVE -> Field(av, { av = it }, "Destination mailbox")
                Act.FLAG -> Field(av, { av = it }, "Flag (e.g. \\Flagged)")
            }

            Button(
                onClick = {
                    viewModel.save(name.trim(), buildWhen(cond, v1.trim(), v2.trim()), buildAction(act, av.trim()))
                },
                enabled = canSave,
                modifier = Modifier.fillMaxWidth(),
            ) { Text("Save rule") }

            Text(
                "Rules sort future mail on the server. Saved rules can be reviewed and " +
                    "their Sieve script previewed under Settings → Rule suggestions.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun <T : Enum<T>> ChipRow(options: List<T>, selected: T, onSelect: (T) -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        options.forEach { opt ->
            FilterChip(
                selected = opt == selected,
                onClick = { onSelect(opt) },
                label = { Text(label(opt)) },
            )
        }
    }
}

private fun label(e: Enum<*>): String = when (e) {
    is Cond -> e.label
    is Act -> e.label
    else -> e.name
}

@Composable
private fun Field(value: String, onChange: (String) -> Unit, label: String) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
    )
}

// --- canonical MatchExpr / Action JSON builders -----------------------------
private fun buildWhen(cond: Cond, v1: String, v2: String): String = when (cond) {
    Cond.FROM_DOMAIN -> {
        val d = if (v1.startsWith("@")) v1 else "@$v1"
        """{"kind":"from_domain_in","domains":[${jstr(d)}]}"""
    }
    Cond.SUBJECT -> """{"kind":"subject_contains_any","needles":[${jstr(v1)}]}"""
    Cond.HEADER -> """{"kind":"header_contains","header":${jstr(v1)},"substring":${jstr(v2)}}"""
}

private fun buildAction(act: Act, v: String): String = when (act) {
    Act.MOVE -> """{"kind":"file_into","folder":${jstr(v)}}"""
    Act.FLAG -> """{"kind":"set_flag","flag":${jstr(v)}}"""
}

/** Minimal JSON string literal (escapes backslash + quote). */
private fun jstr(s: String): String = "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
