// ============================================================================
// data/model/ConnectResult.kt  (DATA group)
// The result of a connect() attempt. DATA-owned — NO Ffi* type (spec §4).
// ============================================================================
package com.plausiden.thundercrab.data.model

/**
 * Outcome of [com.plausiden.thundercrab.data.ThunderCrabRepository.connect].
 *
 * The Repository never leaks the underlying ThunderCrabClient; it reports only
 * success or a classified failure. UI/VM switch on [Failure.kind] (an
 * [ErrorKind]), never on FfiException.
 */
sealed interface ConnectResult {
    /** LOGIN succeeded; the Repository now holds a live client. */
    data object Connected : ConnectResult

    /** LOGIN or transport failed; [kind] is the mapped category, [message] is human-readable. */
    data class Failure(val kind: ErrorKind, val message: String) : ConnectResult
}
