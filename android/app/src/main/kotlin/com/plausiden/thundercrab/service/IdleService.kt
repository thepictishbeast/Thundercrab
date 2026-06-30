// ============================================================================
// service/IdleService.kt  (SERVICE group)
// Foreground service holding a dedicated IMAP IDLE connection — push mail, not
// polling. On server activity it posts a "new mail" notification. Reconnects
// headless using the persisted account (AppPrefs) + the Keystore-encrypted
// password (KeystoreCredentialStore) — owner-approved background credential use.
//
// COMPILES + WIRED. Runtime behavior (notification delivery, doze/standby,
// reconnection, Android-14 foreground-service-type enforcement, POST_NOTIFICATIONS)
// requires ON-DEVICE validation — it cannot be verified on a headless host.
// ============================================================================
package com.plausiden.thundercrab.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.plausiden.thundercrab.data.AppPrefs
import com.plausiden.thundercrab.data.KeystoreCredentialStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import uniffi.thundercrab_ffi.FfiAccountConfig
import uniffi.thundercrab_ffi.FfiCryptoMode
import uniffi.thundercrab_ffi.FfiIdleEvent
import uniffi.thundercrab_ffi.connectIdle

class IdleService : Service() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    @Volatile
    private var running = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        ensureChannels()
        startForeground(ONGOING_ID, ongoingNotification())
        if (!running) {
            running = true
            scope.launch { runIdleLoop() }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        running = false
        scope.cancel()
        super.onDestroy()
    }

    /** Connect, park in IDLE, re-arm forever; reconnect with backoff on failure. */
    private suspend fun runIdleLoop() {
        val account = AppPrefs(applicationContext).loadAccount() ?: return stopGracefully()
        val password = KeystoreCredentialStore(applicationContext).load(account.username)
            ?: return stopGracefully()
        val cfg = try {
            FfiAccountConfig(
                imapHost = account.imapHost, imapPort = account.imapPort.toUShort(),
                smtpHost = account.smtpHost, smtpPort = account.smtpPort.toUShort(),
                sievePort = account.sievePort.toUShort(), username = account.username,
                cryptoMode = FfiCryptoMode.PQ_HYBRID,
            )
        } catch (_: NumberFormatException) {
            return stopGracefully()
        }

        var backoffMs = INITIAL_BACKOFF_MS
        while (scope.isActive && running) {
            try {
                val watcher = connectIdle(cfg, password, "INBOX")
                backoffMs = INITIAL_BACKOFF_MS // reset after a healthy connect
                try {
                    while (scope.isActive && running) {
                        if (watcher.waitRearm() == FfiIdleEvent.ACTIVITY) notifyNewMail()
                    }
                } finally {
                    runCatching { watcher.logout() }
                    runCatching { watcher.close() }
                }
            } catch (_: Exception) {
                // Dropped connection / auth failure — back off and retry.
                delay(backoffMs)
                backoffMs = (backoffMs * 2).coerceAtMost(MAX_BACKOFF_MS)
            }
        }
    }

    private fun stopGracefully() {
        running = false
        @Suppress("DEPRECATION")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            stopForeground(STOP_FOREGROUND_REMOVE)
        } else {
            stopForeground(true)
        }
        stopSelf()
    }

    private fun ensureChannels() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ONGOING,
                "Background mail watch",
                NotificationManager.IMPORTANCE_LOW,
            ).apply { description = "Keeps a connection open to receive mail instantly." },
        )
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_MAIL, "New mail", NotificationManager.IMPORTANCE_DEFAULT),
        )
    }

    private fun ongoingNotification(): Notification =
        NotificationCompat.Builder(this, CHANNEL_ONGOING)
            .setContentTitle("ThunderCrab")
            .setContentText("Watching for new mail")
            .setSmallIcon(android.R.drawable.ic_dialog_email)
            .setOngoing(true)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .build()

    private fun notifyNewMail() {
        val notification = NotificationCompat.Builder(this, CHANNEL_MAIL)
            .setContentTitle("New mail")
            .setContentText("You have new messages in your inbox.")
            .setSmallIcon(android.R.drawable.ic_dialog_email)
            .setAutoCancel(true)
            .build()
        getSystemService(NotificationManager::class.java).notify(MAIL_ID, notification)
    }

    companion object {
        private const val CHANNEL_ONGOING = "thundercrab_idle_ongoing"
        private const val CHANNEL_MAIL = "thundercrab_new_mail"
        private const val ONGOING_ID = 1001
        private const val MAIL_ID = 1002
        private const val INITIAL_BACKOFF_MS = 5_000L
        private const val MAX_BACKOFF_MS = 5L * 60_000L

        /** Start (or no-op if already running) the background IDLE watch. */
        fun start(context: Context) {
            val intent = Intent(context, IdleService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        /** Stop the background IDLE watch. */
        fun stop(context: Context) {
            context.stopService(Intent(context, IdleService::class.java))
        }
    }
}
