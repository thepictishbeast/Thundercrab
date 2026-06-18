// data/MailHeaders.kt (DATA group) — pure From-header parsing for the
// features-only FlagEvent. `internal` so JVM unit tests exercise it directly.
// AVP-2: UNVERIFIED — UNSAFE — nothing here is SHIP-DECISION.
package com.plausiden.thundercrab.data

/**
 * Extract the sender domain (with a leading "@", lowercased) from a raw `From`
 * header value, for [uniffi.thundercrab_ffi.FfiFlagEvent.fromDomainWithAt].
 * Returns "" when the value isn't parseable into a domain — matching that
 * field's documented contract ("Empty if not parseable").
 *
 * Handles the shapes a real From header takes:
 *   - `"Display Name" <user@host.com>`  -> "@host.com"
 *   - bare `user@host.com`               -> "@host.com"
 *   - `user@host.com (Comment)`          -> "@host.com"   (RFC 822 comment dropped)
 *   - display name containing '@'        -> uses the address, not the display name
 *   - no '@' / empty / trailing junk     -> ""
 *
 * Lowercasing normalizes case so "@GitHub.com" and "@github.com" group as one
 * domain in the federated-learning derivation (domains are case-insensitive).
 */
internal fun senderDomainWithAt(rawFrom: String): String {
    // Prefer the angle-bracket address (`Display <addr>`); else the whole value.
    val addr = Regex("<([^>]*)>").find(rawFrom)?.groupValues?.get(1) ?: rawFrom
    // Drop RFC 822 comments "(...)" then trim.
    val cleaned = addr.replace(Regex("\\([^)]*\\)"), "").trim()
    val at = cleaned.lastIndexOf('@')
    if (at < 0 || at == cleaned.length - 1) return ""
    // Domain = everything after the last '@', cut at the first separator.
    val domain = cleaned.substring(at + 1)
        .trim()
        .takeWhile { it != ' ' && it != ',' && it != ';' && it != '>' }
    return if (domain.isEmpty()) "" else "@" + domain.lowercase()
}
