package com.plausiden.thundercrab

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.thundercrab_ffi.plausidenAccountConfig
import uniffi.thundercrab_ffi.thundercrabVersion

/**
 * On-device proof that the Rust core (libthundercrab_ffi.so) loads and executes
 * through JNA on a real Android runtime. If the native library failed to load,
 * the first FFI call throws UnsatisfiedLinkError before any assertion runs —
 * which is exactly the failure mode that compiling and signing cannot catch.
 *
 * AVP-2: this is a smoke test of the FFI seam, not a SHIP-DECISION gate.
 */
@RunWith(AndroidJUnit4::class)
class FfiOnDeviceTest {

    @Test
    fun nativeLibraryLoadsAndVersionResolves() {
        val version = thundercrabVersion()
        assertTrue("version should be non-blank", version.isNotBlank())
    }

    @Test
    fun plausidenAccountConfigMarshalsAcrossFfi() {
        // Exercises record return + UShort marshaling across the FFI boundary.
        val cfg = plausidenAccountConfig("william@plausiden.com")
        assertEquals("mail.plausiden.com", cfg.imapHost)
        assertEquals(993, cfg.imapPort.toInt())
        assertEquals(587, cfg.smtpPort.toInt())
        assertEquals(4190, cfg.sievePort.toInt())
        assertEquals("william@plausiden.com", cfg.username)
    }
}
