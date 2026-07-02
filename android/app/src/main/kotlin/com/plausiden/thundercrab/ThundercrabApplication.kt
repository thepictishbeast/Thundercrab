package com.plausiden.thundercrab

import android.app.Application
import com.plausiden.thundercrab.di.AppContainer

/**
 * Application entry point. Owns the manual ServiceLocator ([AppContainer]) for the
 * whole process lifetime (build spec §1.1: no Hilt — the pinned version matrix has
 * no DI library, and we do not add one).
 *
 * Reachable from any Context via:
 *   (context.applicationContext as ThunderCrabApp).container
 *
 * NOTE: The manifest registers this class as `android:name=".ThunderCrabApp"`.
 * The file/source name is `ThundercrabApplication.kt` (per the file assignment),
 * but the public class name is [ThunderCrabApp] to match the manifest contract.
 *
 * AVP-2: UNVERIFIED — UNSAFE — nothing here is SHIP-DECISION.
 */
class ThunderCrabApp : Application() {

    /** App-lifetime ServiceLocator. Holds the single repository + dbPath. */
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        container = AppContainer(applicationContext)
        // Route Rust-core `tracing` to logcat (tag "ThunderCrab") as early as
        // possible so connect/fetch logs are captured from the first operation.
        // The subscriber installs once per process; the verbose flag is read
        // here, so toggling it in Settings applies on the next launch.
        uniffi.thundercrab_ffi.initLogging(container.prefs.verboseLogging)
    }
}

/**
 * Frozen-contract bridge. The MANIFEST group requirement mandates the Application
 * class be named [ThunderCrabApp], but spec §1.1 and the frozen ViewModel contracts
 * cast via `(applicationContext as ThundercrabApplication).container`. This alias
 * makes both names resolve to the single Application subclass, so UI/VM code that
 * follows the frozen example compiles unchanged. Zero runtime cost.
 */
typealias ThundercrabApplication = ThunderCrabApp
