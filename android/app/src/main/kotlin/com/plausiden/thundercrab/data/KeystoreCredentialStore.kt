// ============================================================================
// data/KeystoreCredentialStore.kt  (DATA group)
// Stores the IMAP password for BACKGROUND re-auth (the IDLE foreground service
// must reconnect without a UI). Encrypted at rest with an AES-256-GCM key that
// lives in the Android Keystore — the key never leaves secure hardware, and the
// ciphertext (+ IV) is what we persist in SharedPreferences.
//
// This is the only credential at rest in the app, and it exists solely so the
// IDLE watcher can reconnect headless (owner-approved). Cleared on logout.
//
// We use the platform Keystore + Cipher (the canonical, documented Android
// pattern) rather than hand-rolling crypto or pulling the deprecated
// androidx.security-crypto library.
// ============================================================================
package com.plausiden.thundercrab.data

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class KeystoreCredentialStore(context: Context) {

    private val sp =
        context.getSharedPreferences("thundercrab_creds", Context.MODE_PRIVATE)

    /** Encrypt and persist `password` for `account`. Overwrites any prior value. */
    fun store(account: String, password: String) {
        val cipher = Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.ENCRYPT_MODE, getOrCreateKey())
        }
        val iv = cipher.iv
        val ciphertext = cipher.doFinal(password.toByteArray(Charsets.UTF_8))
        sp.edit()
            .putString(ivKey(account), iv.toB64())
            .putString(ctKey(account), ciphertext.toB64())
            .apply()
    }

    /** Decrypt the stored password for `account`, or null if none / on failure. */
    fun load(account: String): String? {
        val iv = sp.getString(ivKey(account), null)?.fromB64() ?: return null
        val ciphertext = sp.getString(ctKey(account), null)?.fromB64() ?: return null
        return try {
            val cipher = Cipher.getInstance(TRANSFORMATION).apply {
                init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(GCM_TAG_BITS, iv))
            }
            String(cipher.doFinal(ciphertext), Charsets.UTF_8)
        } catch (_: Exception) {
            // Key invalidated, tampered ciphertext, etc. — treat as "no credential".
            null
        }
    }

    /** Forget the stored credential for `account` (called on logout). */
    fun clear(account: String) {
        sp.edit().remove(ivKey(account)).remove(ctKey(account)).apply()
    }

    fun has(account: String): Boolean = sp.contains(ctKey(account))

    private fun getOrCreateKey(): SecretKey {
        val ks = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (ks.getEntry(KEY_ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }

        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                // No setUserAuthenticationRequired: the background IDLE service
                // must decrypt without a foreground unlock prompt.
                .build(),
        )
        return generator.generateKey()
    }

    private fun ivKey(account: String) = "iv_$account"
    private fun ctKey(account: String) = "ct_$account"

    private fun ByteArray.toB64(): String = Base64.encodeToString(this, Base64.NO_WRAP)
    private fun String.fromB64(): ByteArray = Base64.decode(this, Base64.NO_WRAP)

    private companion object {
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val KEY_ALIAS = "thundercrab_idle_cred"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val GCM_TAG_BITS = 128
    }
}
