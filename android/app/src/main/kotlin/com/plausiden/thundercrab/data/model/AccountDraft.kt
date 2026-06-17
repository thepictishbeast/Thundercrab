// ============================================================================
// data/model/AccountDraft.kt  (DATA group)
// The account-setup form model. DATA-owned domain model — NO Ffi* type.
// Ports are held as Strings because they bind directly to text fields; the
// Repository parses them into FFI UShort on connect (spec §4).
// ============================================================================
package com.plausiden.thundercrab.data.model

/**
 * Editable account configuration backing the Account Setup screen.
 *
 * Host/ports are prefilled by the core via
 * [com.plausiden.thundercrab.data.ThunderCrabRepository.accountDraftFor]; they are
 * never hardcoded in Kotlin. Ports are Strings for the form and converted to
 * UShort at the FFI seam.
 *
 * No credential is held here — the password lives only in transient ViewModel
 * state and is passed straight into `connect` (no-secrets invariant, spec §5.2).
 */
data class AccountDraft(
    val imapHost: String,
    val imapPort: String,
    val smtpHost: String,
    val smtpPort: String,
    val sievePort: String,
    val username: String,
)
