package com.plausiden.thundercrab

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import com.plausiden.thundercrab.navigation.ThundercrabNavHost
import com.plausiden.thundercrab.ui.theme.ThundercrabTheme

/**
 * Single-activity host. Sets the Compose content: the [ThundercrabTheme] (AMOLED
 * dark default, build spec §3) wrapping the [ThundercrabNavHost] (4-screen graph,
 * build spec §2.1).
 *
 * AVP-2: UNVERIFIED — UNSAFE — nothing here is SHIP-DECISION.
 */
class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent {
            ThundercrabTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background,
                ) {
                    ThundercrabNavHost()
                }
            }
        }
    }
}
