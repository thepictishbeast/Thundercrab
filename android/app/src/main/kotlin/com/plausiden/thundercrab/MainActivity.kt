package com.plausiden.thundercrab

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.launch
import com.plausiden.thundercrab.data.AppPrefs
import com.plausiden.thundercrab.data.ThemeMode
import com.plausiden.thundercrab.navigation.ThundercrabNavHost
import com.plausiden.thundercrab.ui.theme.ThundercrabTheme

/**
 * Single-activity host. Drives [ThundercrabTheme] from persisted appearance
 * prefs (so Settings re-themes the app live), re-applies the persisted
 * diagnostics choice at startup (the Rust master switch is in-memory), and
 * shows the first-run diagnostics notice. AVP-2: UNVERIFIED.
 */
class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val container = (application as ThundercrabApplication).container
        val prefs = container.prefs
        // Re-apply the user's persisted diagnostics choice (Rust switch resets on).
        container.repository.setTelemetryEnabled(prefs.telemetryEnabled)

        setContent {
            val appearance by prefs.appearance.collectAsStateWithLifecycle()
            val dark = when (appearance.themeMode) {
                ThemeMode.SYSTEM -> isSystemInDarkTheme()
                ThemeMode.LIGHT -> false
                ThemeMode.DARK -> true
            }
            ThundercrabTheme(
                darkTheme = dark,
                amoled = appearance.amoled,
                dynamicColor = appearance.dynamicColor,
            ) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background,
                ) {
                    ThundercrabNavHost()
                    FirstRunConsent(
                        prefs = prefs,
                        setEnabled = { container.repository.setTelemetryEnabled(it) },
                    )
                }
            }
        }
    }

    /** Flush buffered diagnostics to the home endpoint when backgrounding
     *  (best-effort; no-op when telemetry is off). */
    override fun onStop() {
        super.onStop()
        val repo = (application as ThundercrabApplication).container.repository
        lifecycleScope.launch { repo.uploadDiagnostics() }
    }
}

@Composable
private fun FirstRunConsent(prefs: AppPrefs, setEnabled: (Boolean) -> Unit) {
    var show by remember { mutableStateOf(!prefs.consentShown) }
    if (!show) return
    AlertDialog(
        onDismissRequest = { /* force an explicit choice */ },
        title = { Text("Diagnostics") },
        text = {
            Text(
                "ThunderCrab can record anonymous, on-device diagnostics (operation " +
                    "timings and error categories — never your messages, folder names, or " +
                    "addresses) to help fix problems. Nothing leaves your device. You can " +
                    "change this anytime in Settings.",
            )
        },
        confirmButton = {
            TextButton(onClick = {
                prefs.telemetryEnabled = true; setEnabled(true); prefs.consentShown = true; show = false
            }) { Text("Enable") }
        },
        dismissButton = {
            TextButton(onClick = {
                prefs.telemetryEnabled = false; setEnabled(false); prefs.consentShown = true; show = false
            }) { Text("No thanks") }
        },
    )
}
