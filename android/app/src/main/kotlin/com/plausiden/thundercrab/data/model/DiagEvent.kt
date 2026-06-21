// ============================================================================
// data/model/DiagEvent.kt  (DATA group)
// A privacy-safe diagnostic event, surfaced to the in-app Diagnostics view.
// Mapped from uniffi.thundercrab_ffi.FfiDiagEvent in the Repository. By
// construction it carries NO folder name, address, subject, body, or raw error
// string — only an enumerated kind/category/code, a static op label, counts,
// and a timestamp (see docs/TELEMETRY.md).
// ============================================================================
package com.plausiden.thundercrab.data.model

data class DiagEvent(
    val kind: String,
    val category: String,
    val code: Int,
    val op: String,
    val count: Long,
    val extra: Long,
    val durationMs: Long,
    val atUnixMs: Long,
)
