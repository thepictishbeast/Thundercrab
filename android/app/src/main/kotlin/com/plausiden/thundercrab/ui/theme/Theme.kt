// ============================================================================
// ui/theme/Theme.kt  (UI group)
// ThundercrabTheme — Material3 dual light/dark with AMOLED true-black default.
//
// Branch order (spec §3):
//   dark + amoled (default)          -> AmoledColors (#000000 background/surface)
//   dark + !amoled                   -> DarkColors
//   dynamicColor && SDK>=S && opt-in -> dynamic{Dark,Light}ColorScheme
//   else                             -> LightColors
//
// Signature is frozen:
//   ThundercrabTheme(darkTheme = isSystemInDarkTheme(), amoled = true,
//                    dynamicColor = false, content)
// AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext

@Composable
fun ThundercrabTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    amoled: Boolean = true,
    dynamicColor: Boolean = false,
    content: @Composable () -> Unit,
) {
    val colorScheme = when {
        // Dynamic color is explicitly opt-in and does NOT pre-empt AMOLED.
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            val ctx = LocalContext.current
            if (darkTheme) dynamicDarkColorScheme(ctx) else dynamicLightColorScheme(ctx)
        }
        darkTheme && amoled -> AmoledColors
        darkTheme -> DarkColors
        else -> LightColors
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = ThundercrabTypography,
        content = content,
    )
}
