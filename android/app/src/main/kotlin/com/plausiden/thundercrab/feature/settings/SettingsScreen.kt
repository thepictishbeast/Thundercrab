// ============================================================================
// feature/settings/SettingsScreen.kt  (UI group)
// Settings hub: Appearance (theme mode / AMOLED / dynamic color — applied live)
// and Privacy & diagnostics (on-device, PII-free telemetry toggle + a view of
// exactly what's recorded). Also links to rule suggestions. AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.settings

import androidx.compose.foundation.clickable
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
import androidx.compose.material.icons.filled.KeyboardArrowRight
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.plausiden.thundercrab.data.ThemeMode

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    viewModel: SettingsViewModel,
    onOpenSuggestions: () -> Unit,
    onCreateRule: () -> Unit,
    onBack: () -> Unit,
) {
    val appearance by viewModel.appearance.collectAsStateWithLifecycle()
    val telemetry by viewModel.telemetry.collectAsStateWithLifecycle()
    val diagnostics by viewModel.diagnostics.collectAsStateWithLifecycle()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings", fontWeight = FontWeight.SemiBold) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
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
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .padding(vertical = 8.dp),
        ) {
            // ---- Appearance ----------------------------------------------------
            SectionHeader("Appearance")
            SettingRow(title = "Theme") {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    ThemeMode.entries.forEach { mode ->
                        FilterChip(
                            selected = appearance.themeMode == mode,
                            onClick = { viewModel.setThemeMode(mode) },
                            label = { Text(mode.label()) },
                        )
                    }
                }
            }
            ToggleRow(
                title = "AMOLED black",
                subtitle = "True-black background on dark themes (saves OLED battery)",
                checked = appearance.amoled,
                onCheckedChange = viewModel::setAmoled,
            )
            ToggleRow(
                title = "Dynamic color",
                subtitle = "Use the system wallpaper palette (Android 12+)",
                checked = appearance.dynamicColor,
                onCheckedChange = viewModel::setDynamicColor,
            )

            HorizontalDivider(Modifier.padding(vertical = 8.dp))

            // ---- Privacy & diagnostics ----------------------------------------
            SectionHeader("Privacy & diagnostics")
            ToggleRow(
                title = "Diagnostics",
                subtitle = "Record anonymous, on-device diagnostics to help debugging. " +
                    "No message content, folder names, or addresses — ever. Nothing leaves " +
                    "this device.",
                checked = telemetry,
                onCheckedChange = viewModel::setTelemetry,
            )
            if (telemetry) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Button(onClick = viewModel::refreshDiagnostics) { Text("Refresh") }
                    Button(onClick = viewModel::clearDiagnostics) { Text("Clear") }
                }
                if (diagnostics.isEmpty()) {
                    Text(
                        text = "No diagnostics recorded yet.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp),
                    )
                } else {
                    Text(
                        text = "Recent events (newest last) — exactly what would be sent:",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp),
                    )
                    diagnostics.takeLast(25).forEach { ev ->
                        val errPart = if (ev.category != "None") " · ${ev.category}" else ""
                        Text(
                            text = "${ev.kind}$errPart · ${ev.op} · n=${ev.count} · ${ev.durationMs}ms",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 20.dp, vertical = 2.dp),
                        )
                    }
                }
            }

            HorizontalDivider(Modifier.padding(vertical = 8.dp))

            // ---- Rules ---------------------------------------------------------
            SectionHeader("Mail rules")
            NavRow(title = "Create a rule", subtitle = "Define where incoming mail goes", onClick = onCreateRule)
            NavRow(title = "Rule suggestions", subtitle = "Review + accept learned sorting rules", onClick = onOpenSuggestions)
        }
    }
}

private fun ThemeMode.label(): String = when (this) {
    ThemeMode.SYSTEM -> "System"
    ThemeMode.LIGHT -> "Light"
    ThemeMode.DARK -> "Dark"
}

@Composable
private fun SectionHeader(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        fontWeight = FontWeight.SemiBold,
        modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 12.dp, bottom = 4.dp),
    )
}

@Composable
private fun SettingRow(title: String, control: @Composable () -> Unit) {
    Column(modifier = Modifier.padding(horizontal = 20.dp, vertical = 10.dp)) {
        Text(title, style = MaterialTheme.typography.bodyLarge)
        Column(modifier = Modifier.padding(top = 8.dp)) { control() }
    }
}

@Composable
private fun ToggleRow(title: String, subtitle: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onCheckedChange(!checked) }
            .padding(horizontal = 20.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}

@Composable
private fun NavRow(title: String, subtitle: String, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Icon(Icons.Filled.KeyboardArrowRight, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
