// ============================================================================
// di/AppContainer.kt  (DATA group)
// Hand-rolled ServiceLocator. The pinned version matrix contains NO DI library
// and we do not add one (Hilt would break the matrix). Spec §1.1.
//
// Constructed once in ThundercrabApplication.onCreate() and reachable via
// (application as ThundercrabApplication).container. It owns:
//   - the single app-lifetime ThunderCrabRepository instance,
//   - the absolute dbPath = "${filesDir.absolutePath}/thundercrab.db".
// ============================================================================
package com.plausiden.thundercrab.di

import android.content.Context
import com.plausiden.thundercrab.data.AppPrefs
import com.plausiden.thundercrab.data.ThunderCrabRepository
import com.plausiden.thundercrab.data.ThunderCrabRepositoryImpl

/**
 * Manual DI container (ServiceLocator). Holds app-lifetime singletons.
 *
 * @param context application context; only `filesDir` is read, and only to
 *                derive the SQLite path. No credential is ever persisted here
 *                (no-secrets invariant, spec §5.2).
 */
class AppContainer(context: Context) {

    /** Absolute SQLite path for rules + features-only flag events (spec §1.1/§5.2). */
    val dbPath: String = "${context.filesDir.absolutePath}/thundercrab.db"

    /** The app's single Repository instance. ViewModels receive this via factories. */
    val repository: ThunderCrabRepository = ThunderCrabRepositoryImpl(dbPath)

    /** On-device appearance + diagnostics-consent prefs (drives live re-theming). */
    val prefs: AppPrefs = AppPrefs(context)
}
