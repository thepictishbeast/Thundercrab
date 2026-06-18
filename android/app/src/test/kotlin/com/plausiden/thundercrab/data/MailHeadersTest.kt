package com.plausiden.thundercrab.data

import org.junit.Assert.assertEquals
import org.junit.Test

/** Unit tests for senderDomainWithAt — From-header -> "@domain" (or "") parsing. */
class MailHeadersTest {

    @Test fun displayNameWithAngleAddress() =
        assertEquals("@host.com", senderDomainWithAt("\"Display Name\" <user@host.com>"))

    @Test fun bareAddress() =
        assertEquals("@github.com", senderDomainWithAt("user@github.com"))

    @Test fun rfc822CommentDropped() =
        assertEquals("@host.com", senderDomainWithAt("user@host.com (Friendly Name)"))

    @Test fun displayNameContainingAtUsesRealAddress() =
        assertEquals("@domain.com", senderDomainWithAt("\"weird@name\" <real@domain.com>"))

    @Test fun lowercasesDomain() =
        assertEquals("@github.com", senderDomainWithAt("Bot <noreply@GitHub.COM>"))

    @Test fun trailingJunkAfterDomainTrimmed() =
        assertEquals("@host.com", senderDomainWithAt("user@host.com extra tokens"))

    @Test fun noAtSignIsUnparseable() =
        assertEquals("", senderDomainWithAt("mailer-daemon"))

    @Test fun emptyIsUnparseable() =
        assertEquals("", senderDomainWithAt(""))

    @Test fun trailingAtIsUnparseable() =
        assertEquals("", senderDomainWithAt("user@"))
}
