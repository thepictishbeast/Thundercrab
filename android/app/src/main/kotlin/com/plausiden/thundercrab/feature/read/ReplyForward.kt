// ============================================================================
// feature/read/ReplyForward.kt  (VIEWMODEL group)
// Pure reply/forward derivations: subject prefixing, quoting, the forwarded
// block, and the RFC 5322 §3.6.4 References chain. No Android, no IO, no Ffi —
// deterministic string functions so they are unit-testable in isolation.
// ============================================================================
package com.plausiden.thundercrab.feature.read

/** Pure helpers for building reply/forward drafts from an original message. */
object ReplyForward {
    // Anchored to the start; recognizes an existing prefix so we don't double it.
    private val REPLY_PREFIX = Regex("^re\\s*:", RegexOption.IGNORE_CASE)
    private val FORWARD_PREFIX = Regex("^(fwd|fw)\\s*:", RegexOption.IGNORE_CASE)

    /** "Re: <subject>", unless it already starts with a Re: prefix (any case). */
    fun replySubject(original: String): String {
        val s = original.trim()
        return if (REPLY_PREFIX.containsMatchIn(s)) s else "Re: $s"
    }

    /** "Fwd: <subject>", unless it already starts with an Fwd:/Fw: prefix. */
    fun forwardSubject(original: String): String {
        val s = original.trim()
        return if (FORWARD_PREFIX.containsMatchIn(s)) s else "Fwd: $s"
    }

    /**
     * Top-posted reply body: two blank lines for the user to type, then an
     * attribution line and the original quoted with "> " (a Markdown blockquote,
     * so it renders as a quote in the ThunderCrab reader).
     */
    fun quotedReply(from: String, date: String, body: String): String {
        val quoted = body.trimEnd('\n')
            .lines()
            .joinToString("\n") { if (it.isEmpty()) ">" else "> $it" }
        val attribution = if (date.isBlank()) "On $from wrote:" else "On $date, $from wrote:"
        return "\n\n$attribution\n$quoted\n"
    }

    /** Forwarded-message block: two blank lines, a header banner, then the body. */
    fun forwardedBody(
        from: String,
        date: String,
        subject: String,
        to: String,
        body: String,
    ): String = buildString {
        append("\n\n---------- Forwarded message ----------\n")
        append("From: $from\n")
        if (date.isNotBlank()) append("Date: $date\n")
        append("Subject: $subject\n")
        if (to.isNotBlank()) append("To: $to\n")
        append("\n")
        append(body.trimEnd('\n'))
        append("\n")
    }

    /**
     * The reply's References header (RFC 5322 §3.6.4): the original References
     * chain with the original Message-ID appended; or just the Message-ID; or
     * just the chain; or null when the original had neither.
     */
    fun referencesChain(originalReferences: String?, originalMessageId: String?): String? = when {
        !originalReferences.isNullOrBlank() && !originalMessageId.isNullOrBlank() ->
            "$originalReferences $originalMessageId"
        !originalMessageId.isNullOrBlank() -> originalMessageId
        !originalReferences.isNullOrBlank() -> originalReferences
        else -> null
    }

    /** Case-insensitive lookup over the (lowercased-name) other-headers list. */
    fun headerValue(headers: List<Pair<String, String>>, name: String): String? =
        headers.firstOrNull { it.first.equals(name, ignoreCase = true) }?.second
}
