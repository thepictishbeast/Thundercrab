// ============================================================================
// ui/theme/Color.kt  (UI group)
// Material3 color schemes for ThunderCrab. Three variants are derived in
// Theme.kt; this file holds the raw palettes:
//   - LightColors   : standard Material3 light scheme
//   - DarkColors    : conventional dark scheme (elevation grays)
//   - AmoledColors  : OLED true-black dark default (background/surface = #000000)
// Identity: Thunderbird's signature blue → cyan gradient, so ThunderCrab reads
// as a member of the Thunderbird family at a glance. No emoji, no business logic.
// ============================================================================
package com.plausiden.thundercrab.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color

// --- Brand seeds (Thunderbird family) ---------------------------------------
// Thunderbird's mark is a blue→cyan gradient. We anchor on the blue and use the
// cyan as the secondary accent, mirroring the wordmark.
internal val TbBlue = Color(0xFF1373D9)          // Thunderbird Blue 50 (primary, light)
internal val TbBlueBright = Color(0xFF3B9DFF)    // brighter blue for dark surfaces
internal val TbBlueDeep = Color(0xFF0B5BB5)      // pressed / container
internal val TbCyan = Color(0xFF1AC2E8)          // gradient tail (secondary accent)
internal val TbCyanLight = Color(0xFF7FE0F2)
internal val TbAmber = Color(0xFFF59E0B)         // tertiary (flags/stars)
internal val CrabRed = Color(0xFFEF4444)
internal val CrabRedLight = Color(0xFFFCA5A5)

// Elevation gray used ONLY where depth genuinely helps (non-AMOLED dark + bars).
internal val Elevation01 = Color(0xFF121417)
internal val Elevation02 = Color(0xFF1B1F24)
internal val TrueBlack = Color(0xFF000000)

// --- Light scheme (standard Material3 light) --------------------------------
val LightColors = lightColorScheme(
    primary = TbBlue,
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFD6E7FF),
    onPrimaryContainer = Color(0xFF001C3A),
    secondary = Color(0xFF0E8FB0),
    onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = Color(0xFFC4ECF7),
    onSecondaryContainer = Color(0xFF002731),
    tertiary = TbAmber,
    onTertiary = Color(0xFF3E2900),
    background = Color(0xFFF8FAFC),
    onBackground = Color(0xFF101418),
    surface = Color(0xFFFFFFFF),
    onSurface = Color(0xFF101418),
    surfaceVariant = Color(0xFFE2E8F0),
    onSurfaceVariant = Color(0xFF44474C),
    outline = Color(0xFF74777C),
    outlineVariant = Color(0xFFCBD5E1),
    error = CrabRed,
    onError = Color(0xFFFFFFFF),
    errorContainer = Color(0xFFFEE2E2),
    onErrorContainer = Color(0xFF410002),
)

// --- Conventional dark scheme (elevation grays, not true black) -------------
val DarkColors = darkColorScheme(
    primary = TbBlueBright,
    onPrimary = Color(0xFF002A52),
    primaryContainer = TbBlueDeep,
    onPrimaryContainer = Color(0xFFD6E7FF),
    secondary = TbCyanLight,
    onSecondary = Color(0xFF00363F),
    secondaryContainer = Color(0xFF034E5C),
    onSecondaryContainer = Color(0xFFC4ECF7),
    tertiary = TbAmber,
    onTertiary = Color(0xFF3E2900),
    background = Elevation01,
    onBackground = Color(0xFFE3E7EB),
    surface = Elevation01,
    onSurface = Color(0xFFE3E7EB),
    surfaceVariant = Elevation02,
    onSurfaceVariant = Color(0xFFC2C7CE),
    outline = Color(0xFF8C9298),
    outlineVariant = Color(0xFF2A2F36),
    error = CrabRedLight,
    onError = Color(0xFF690005),
    errorContainer = Color(0xFF93000A),
    onErrorContainer = Color(0xFFFEE2E2),
)

// --- AMOLED true-black dark default -----------------------------------------
// Background & surface are #000000 so OLED pixels are physically off
// (battery + contrast doctrine). Elevation grays only where depth helps.
val AmoledColors = darkColorScheme(
    primary = TbBlueBright,
    onPrimary = Color(0xFF002A52),
    primaryContainer = TbBlueDeep,
    onPrimaryContainer = Color(0xFFD6E7FF),
    secondary = TbCyanLight,
    onSecondary = Color(0xFF00363F),
    secondaryContainer = Color(0xFF034E5C),
    onSecondaryContainer = Color(0xFFC4ECF7),
    tertiary = TbAmber,
    onTertiary = Color(0xFF3E2900),
    background = TrueBlack,
    onBackground = Color(0xFFE6E6E6),
    surface = TrueBlack,
    onSurface = Color(0xFFE6E6E6),
    surfaceVariant = Elevation01,
    onSurfaceVariant = Color(0xFFB8BCC2),
    outline = Color(0xFF6B7177),
    outlineVariant = Color(0xFF1B1F24),
    error = CrabRedLight,
    onError = Color(0xFF690005),
    errorContainer = Color(0xFF5C0006),
    onErrorContainer = Color(0xFFFEE2E2),
)
