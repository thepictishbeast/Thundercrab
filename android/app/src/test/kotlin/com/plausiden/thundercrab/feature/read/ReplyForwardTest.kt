package com.plausiden.thundercrab.feature.read

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Unit tests for the pure reply/forward derivations (RFC 5322 §3.6.4). */
class ReplyForwardTest {

    // --- Subject prefixing ---------------------------------------------------

    @Test fun replyAddsRePrefix() =
        assertEquals("Re: Hello", ReplyForward.replySubject("Hello"))

    @Test fun replyDoesNotDoubleExistingRe() =
        assertEquals("Re: Hello", ReplyForward.replySubject("Re: Hello"))

    @Test fun replyRecognizesExistingReAnyCase() =
        assertEquals("RE: Hello", ReplyForward.replySubject("RE: Hello"))

    @Test fun replyToForwardStillAddsRe() =
        assertEquals("Re: Fwd: Hello", ReplyForward.replySubject("Fwd: Hello"))

    @Test fun forwardAddsFwdPrefix() =
        assertEquals("Fwd: Hello", ReplyForward.forwardSubject("Hello"))

    @Test fun forwardDoesNotDoubleFwd() =
        assertEquals("Fwd: Hello", ReplyForward.forwardSubject("Fwd: Hello"))

    @Test fun forwardRecognizesShortFwPrefix() =
        assertEquals("Fw: Hello", ReplyForward.forwardSubject("Fw: Hello"))

    // --- References chain ----------------------------------------------------

    @Test fun referencesAppendsMessageIdToChain() =
        assertEquals(
            "<a@h> <b@h> <c@h>",
            ReplyForward.referencesChain("<a@h> <b@h>", "<c@h>"),
        )

    @Test fun referencesFallsBackToMessageIdAlone() =
        assertEquals("<c@h>", ReplyForward.referencesChain(null, "<c@h>"))

    @Test fun referencesFallsBackToChainAlone() =
        assertEquals("<a@h>", ReplyForward.referencesChain("<a@h>", null))

    @Test fun referencesNullWhenNeither() =
        assertNull(ReplyForward.referencesChain(null, null))

    // --- Quoting -------------------------------------------------------------

    @Test fun quotedReplyPrefixesEachLine() {
        val out = ReplyForward.quotedReply("a@h", "Wed, 2 Jul 2026", "line1\nline2")
        assertTrue(out.contains("On Wed, 2 Jul 2026, a@h wrote:"))
        assertTrue(out.contains("> line1"))
        assertTrue(out.contains("> line2"))
    }

    @Test fun quotedReplyOmitsDateWhenBlank() {
        val out = ReplyForward.quotedReply("a@h", "", "x")
        assertTrue(out.contains("On a@h wrote:"))
    }

    @Test fun forwardedBodyHasBanner() {
        val out = ReplyForward.forwardedBody("a@h", "d", "subj", "b@h", "body")
        assertTrue(out.contains("---------- Forwarded message ----------"))
        assertTrue(out.contains("From: a@h"))
        assertTrue(out.contains("Subject: subj"))
        assertTrue(out.contains("body"))
    }

    // --- Header lookup -------------------------------------------------------

    @Test fun headerValueIsCaseInsensitive() {
        val headers = listOf("message-id" to "<x@h>", "references" to "<a@h>")
        assertEquals("<x@h>", ReplyForward.headerValue(headers, "Message-ID"))
        assertNull(ReplyForward.headerValue(headers, "reply-to"))
    }
}
