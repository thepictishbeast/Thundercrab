package com.plausiden.thundercrab

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.plausiden.thundercrab.data.ThunderCrabRepositoryImpl
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.thundercrab_ffi.FfiFlagEvent
import uniffi.thundercrab_ffi.FfiFlagSource
import uniffi.thundercrab_ffi.recordFlagEvent

/**
 * On-device proof of the OFFLINE server-coupling flow through the actual app
 * Repository (not just the raw FFI): seed flag events -> previewSuggestions
 * (derive + safety gate) -> acceptSuggestion (save) -> savedRulesSieve (emit).
 * No network, no live mailbox; the live ManageSieve push is never exercised.
 *
 * This complements FfiOnDeviceTest (raw FFI) by covering the Kotlin Repository
 * seam: the suspend wrappers, IO dispatch, and FfiCrabRule -> RuleSuggestion
 * mapping (incl. the "Move to <folder>" summary).
 */
@RunWith(AndroidJUnit4::class)
class SuggestionsFlowTest {

    private fun event(seed: Int, dest: String): FfiFlagEvent {
        val hash = ByteArray(32).also { it[0] = seed.toByte(); it[1] = (seed ushr 8).toByte() }
        return FfiFlagEvent(
            messageHash = hash,
            source = FfiFlagSource.MANUAL_MOVE,
            destination = dest,
            fromDomainWithAt = "@github.com",
            listId = null,
            hasListUnsubscribe = false,
            subjectTokens = emptyList(),
            priorityHigh = false,
            observedAt = 1_700_000_000L + seed,
        )
    }

    @Test
    fun offlineFlow_seedToSuggestionToSavedSieve() = runBlocking<Unit> {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val dbPath = "${ctx.cacheDir.absolutePath}/sugg-flow-${System.nanoTime()}.db"
        java.io.File(dbPath).delete()
        val repo = ThunderCrabRepositoryImpl(dbPath)

        // Dominant pattern: 9/10 of @github.com moved to "Dev"; one stray to INBOX
        // (dropped by min-obs + the gate's no-file-into-INBOX rule).
        repeat(9) { recordFlagEvent(dbPath, event(it, "Dev")) }
        recordFlagEvent(dbPath, event(200, "INBOX"))

        val suggestions = repo.previewSuggestions().getOrThrow()
        val dev = suggestions.firstOrNull { it.summary == "Move to Dev" }
        assertTrue("expected a 'Move to Dev' suggestion, got $suggestions", dev != null)

        repo.acceptSuggestion(dev!!.id).getOrThrow()
        val saved = repo.loadSavedRules().getOrThrow()
        assertTrue("accepted rule should be saved", saved.any { it.id == dev.id })

        val sieve = repo.savedRulesSieve().getOrThrow()
        assertTrue("emitted Sieve should fileinto Dev:\n$sieve", sieve.contains("fileinto :create \"Dev\""))
        assertTrue("emitted Sieve should match the github domain:\n$sieve",
            sieve.contains("address :domain :is \"from\" [\"github.com\"]"))
        assertEquals("stray INBOX event must not become a rule", false, sieve.contains("INBOX"))

        java.io.File(dbPath).delete()
    }
}
