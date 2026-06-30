// ============================================================================
// ui/components/MessageBodyHtml.kt  (UI group)
// Renders pre-sanitized message HTML in a maximally locked-down WebView.
//
// The HTML is ALREADY sanitized by the Rust core (no scripts, no remote
// content). This WebView is defense-in-depth: even if a sanitizer miss slipped
// through, JavaScript is OFF and all network loads are BLOCKED, so nothing can
// execute or phone home. Loaded with a null base URL (no origin).
// ============================================================================
package com.plausiden.thundercrab.ui.components

import android.annotation.SuppressLint
import android.webkit.WebView
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView

@SuppressLint("SetJavaScriptEnabled")
@Composable
fun MessageBodyHtml(html: String, modifier: Modifier = Modifier) {
    AndroidView(
        modifier = modifier,
        factory = { ctx ->
            WebView(ctx).apply {
                with(settings) {
                    javaScriptEnabled = false
                    blockNetworkLoads = true
                    loadsImagesAutomatically = false
                    allowFileAccess = false
                    allowContentAccess = false
                    @Suppress("DEPRECATION")
                    allowFileAccessFromFileURLs = false
                    @Suppress("DEPRECATION")
                    allowUniversalAccessFromFileURLs = false
                }
                isVerticalScrollBarEnabled = true
            }
        },
        update = { web ->
            // Null base URL => no origin, no relative resource resolution.
            web.loadDataWithBaseURL(null, html, "text/html", "utf-8", null)
        },
    )
}
