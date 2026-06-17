// ============================================================================
// ui/theme/Color.kt  (UI group)
// Material3 color schemes for ThunderCrab. Three dark variants are derived in
// Theme.kt; this file holds the raw palettes:
//   - LightColors   : standard Material3 light scheme
//   - DarkColors    : conventional dark scheme (elevation grays)
//   - AmoledColors  : OLED true-black dark default (background/surface = #000000)
// No emoji, no business logic. AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color

// --- Brand seeds ------------------------------------------------------------
// A restrained, premium teal/indigo pairing. Kept deliberately calm so the
// AMOLED dark default reads as confident rather than neon.
internal val CrabTeal = Color(0xFF4FD1C5)
internal val CrabTealDark = Color(0xFF0D9488)
internal val CrabIndigo = Color(0xFF6366F1)
internal val CrabIndigoLight = Color(0xFFA5B4FC)
internal val CrabAmber = Color(0xFFF59E0B)
internal val CrabRed = Color(0xFFEF4444)
internal val CrabRedLight = Color(0xFFFCA5A5)

// Elevation gray used ONLY where depth genuinely helps (non-AMOLED dark + bars).
internal val Elevation01 = Color(0xFF121212)
internal val Elevation02 = Color(0xFF1E1E1E)
internal val TrueBlack = Color(0xFF000000)

// --- Light scheme (standard Material3 light) --------------------------------
val LightColors = lightColorScheme(
    primary = CrabTealDark,
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFB2F5EA),
    onPrimaryContainer = Color(0xFF00201C),
    secondary = CrabIndigo,
    onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = Color(0xFFE0E7FF),
    onSecondaryContainer = Color(0xFF1E1B4B),
    tertiary = CrabAmber,
    onTertiary = Color(0xFF3E2900),
    background = Color(0xFFFBFBFC),
    onBackground = Color(0xFF1A1C1B),
    surface = Color(0xFFFBFBFC),
    onSurface = Color(0xFF1A1C1B),
    surfaceVariant = Color(0xFFE6E9E8),
    onSurfaceVariant = Color(0xFF424847),
    outline = Color(0xFF727876),
    error = CrabRed,
    onError = Color(0xFFFFFFFF),
    errorContainer = Color(0xFFFEE2E2),
    onErrorContainer = Color(0xFF410002),
)

// --- Conventional dark scheme (elevation grays, not true black) -------------
val DarkColors = darkColorScheme(
    primary = CrabTeal,
    onPrimary = Color(0xFF00382F),
    primaryContainer = CrabTealDark,
    onPrimaryContainer = Color(0xFFB2F5EA),
    secondary = CrabIndigoLight,
    onSecondary = Color(0xFF1E1B4B),
    secondaryContainer = Color(0xFF3730A3),
    onSecondaryContainer = Color(0xFFE0E7FF),
    tertiary = CrabAmber,
    onTertiary = Color(0xFF3E2900),
    background = Elevation01,
    onBackground = Color(0xFFE3E3E1),
    surface = Elevation01,
    onSurface = Color(0xFFE3E3E1),
    surfaceVariant = Elevation02,
    onSurfaceVariant = Color(0xFFC4C7C5),
    outline = Color(0xFF8C9290),
    error = CrabRedLight,
    onError = Color(0xFF690005),
    errorContainer = Color(0xFF93000A),
    onErrorContainer = Color(0xFFFEE2E2),
)

// --- AMOLED true-black dark default -----------------------------------------
// Background & surface are #000000 so OLED pixels are physically off
// (battery + contrast doctrine). Elevation grays only on surfaceVariant where
// depth genuinely helps (e.g. the top app bar).
val AmoledColors = darkColorScheme(
    primary = CrabTeal,
    onPrimary = Color(0xFF00382F),
    primaryContainer = CrabTealDark,
    onPrimaryContainer = Color(0xFFB2F5EA),
    secondary = CrabIndigoLight,
    onSecondary = Color(0xFF1E1B4B),
    secondaryContainer = Color(0xFF3730A3),
    onSecondaryContainer = Color(0xFFE0E7FF),
    tertiary = CrabAmber,
    onTertiary = Color(0xFF3E2900),
    background = TrueBlack,
    onBackground = Color(0xFFE6E6E6),
    surface = TrueBlack,
    onSurface = Color(0xFFE6E6E6),
    surfaceVariant = Elevation01,
    onSurfaceVariant = Color(0xFFB8BCBA),
    outline = Color(0xFF6B716F),
    error = CrabRedLight,
    onError = Color(0xFF690005),
    errorContainer = Color(0xFF5C0006),
    onErrorContainer = Color(0xFFFEE2E2),
)
