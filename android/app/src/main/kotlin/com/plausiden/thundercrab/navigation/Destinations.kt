// ============================================================================
// navigation/Destinations.kt  (UI group)
// Route constants + arg keys + small URL-encode helpers for the nav graph.
// Routes (spec §2.1):
//   setup → folders → messages/{folder} → read/{folder}/{uid}
// Folder names are URL-encoded as a nav arg; uid is an Int nav arg (converted
// to UInt at the FFI seam inside the Repository — never here). AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.navigation

import android.net.Uri

object Destinations {
    const val SETUP = "setup"
    const val FOLDERS = "folders"
    const val SUGGESTIONS = "suggestions"

    // Arg keys.
    const val ARG_FOLDER = "folder"
    const val ARG_UID = "uid"

    // Route templates (with arg placeholders) for NavHost composable() declarations.
    const val MESSAGES_ROUTE = "messages/{$ARG_FOLDER}"
    const val READ_ROUTE = "read/{$ARG_FOLDER}/{$ARG_UID}"

    /** Build a concrete messages route, URL-encoding the folder name. */
    fun messages(folder: String): String = "messages/${encode(folder)}"

    /** Build a concrete read route, URL-encoding the folder name. */
    fun read(folder: String, uid: Int): String = "read/${encode(folder)}/$uid"

    /** Decode a folder nav arg back to its raw IMAP name. */
    fun decodeFolder(raw: String): String = Uri.decode(raw)

    private fun encode(value: String): String = Uri.encode(value)
}
