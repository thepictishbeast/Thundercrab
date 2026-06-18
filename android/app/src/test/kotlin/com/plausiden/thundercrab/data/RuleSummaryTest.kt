package com.plausiden.thundercrab.data

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Unit tests for summarizeRuleAction — the pure Action-JSON -> human-label logic
 * behind each RuleSuggestion shown on the Suggestions screen. Runs on the JVM
 * (no emulator); needs the real org.json testImplementation dependency.
 *
 * The Action JSON shape is the serde-tagged form emitted by thundercrab-core
 * (crab_rule.rs): {"kind":"file_into","folder":...} etc.
 */
class RuleSummaryTest {

    @Test
    fun fileInto_movesToFolder() {
        assertEquals(
            "Move to Promotions",
            summarizeRuleAction("""{"kind":"file_into","folder":"Promotions"}""", "fallback"),
        )
    }

    @Test
    fun setFlag_flagsAs() {
        assertEquals(
            "Flag as \\Flagged",
            summarizeRuleAction("""{"kind":"set_flag","flag":"\\Flagged"}""", "fallback"),
        )
    }

    @Test
    fun sequence_usesFirstActionAndMarksMore() {
        val json = """
            {"kind":"sequence","actions":[
              {"kind":"set_flag","flag":"\\Seen"},
              {"kind":"file_into","folder":"Lists"}
            ]}
        """.trimIndent()
        assertEquals("Flag as \\Seen …", summarizeRuleAction(json, "fallback"))
    }

    @Test
    fun sequence_singleAction_noEllipsis() {
        val json = """{"kind":"sequence","actions":[{"kind":"file_into","folder":"Dev"}]}"""
        assertEquals("Move to Dev", summarizeRuleAction(json, "fallback"))
    }

    @Test
    fun malformedJson_fallsBack() {
        assertEquals("fallback", summarizeRuleAction("not json", "fallback"))
    }

    @Test
    fun unknownKind_fallsBack() {
        assertEquals("fallback", summarizeRuleAction("""{"kind":"discard"}""", "fallback"))
    }

    @Test
    fun fileInto_blankFolder_fallsBack() {
        assertEquals("fallback", summarizeRuleAction("""{"kind":"file_into","folder":""}""", "fallback"))
    }
}
