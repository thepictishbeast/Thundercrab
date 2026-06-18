// ============================================================================
// ui/components/SenderAvatar.kt  (UI group)
// A colored circular avatar with the sender's initial — the Thunderbird-style
// leading element of a message row. Color is derived deterministically from the
// seed (sender address) so the same sender always gets the same hue. No emoji,
// no network, no business logic.
// ============================================================================
package com.plausiden.thundercrab.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlin.math.abs

/** A 42dp circular avatar showing `seed`'s first alphanumeric initial. */
@Composable
fun SenderAvatar(seed: String, modifier: Modifier = Modifier) {
    val initial = seed.trim()
        .firstOrNull { it.isLetterOrDigit() }
        ?.uppercaseChar()
        ?.toString()
        ?: "?"
    Box(
        modifier = modifier
            .size(42.dp)
            .clip(CircleShape)
            .background(avatarColor(seed)),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = initial,
            color = Color.White,
            fontWeight = FontWeight.SemiBold,
            style = MaterialTheme.typography.titleMedium,
        )
    }
}

// A calm, legible palette (white text passes contrast on every entry).
private val avatarPalette = listOf(
    Color(0xFF1373D9), Color(0xFF0E8FB0), Color(0xFF6D4AC4), Color(0xFFC2185B),
    Color(0xFF00897B), Color(0xFFE65100), Color(0xFF3949AB), Color(0xFF2E7D32),
)

private fun avatarColor(seed: String): Color {
    if (seed.isEmpty()) return avatarPalette[0]
    val h = abs(seed.fold(7) { acc, c -> acc * 31 + c.code })
    return avatarPalette[h % avatarPalette.size]
}
