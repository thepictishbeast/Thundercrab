// ThunderCrab Android — root build script (BUILD group). AVP-2: UNVERIFIED — UNSAFE.
// All version pins live in gradle/libs.versions.toml. Plugins are declared here
// `apply false` so the :app module applies them with versions from the catalog.
plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.android) apply false
    alias(libs.plugins.compose.compiler) apply false
}
