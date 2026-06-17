// ============================================================================
// data/model/ErrorKind.kt  (DATA group)
// The app-facing error taxonomy. DATA-owned — NO Ffi* type crosses this line.
// The Repository catches FfiException subclasses and maps each to an ErrorKind;
// ViewModels and UI switch on ErrorKind and never catch FfiException (spec §4).
// ============================================================================
package com.plausiden.thundercrab.data.model

/**
 * Stable error categories surfaced to the UI, mapped 1:1 from the FFI's
 * FfiException subclasses inside the Repository.
 *
 * - [TRANSPORT]       TCP / TLS / greeting failure.
 * - [AUTH]            LOGIN rejected.
 * - [PROTOCOL]        IMAP protocol or store/DB error.
 * - [NOT_IMPLEMENTED] gated/unimplemented surface (e.g. a logged-out client).
 * - [INVALID_INPUT]   malformed input at the FFI seam (bad port, bad hash, …).
 * - [UNKNOWN]         anything the Repository could not classify.
 */
enum class ErrorKind {
    TRANSPORT,
    AUTH,
    PROTOCOL,
    NOT_IMPLEMENTED,
    INVALID_INPUT,
    UNKNOWN,
}
