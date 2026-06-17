// ============================================================================
// feature/folders/FolderListUiState.kt  (VIEWMODEL group)
// Sealed-interface UI contract for the Folder List screen. Uses DATA domain
// model Folder + ErrorKind only; no Ffi* type. Spec §4 / VIEWMODEL contracts.
// ============================================================================
package com.plausiden.thundercrab.feature.folders

import com.plausiden.thundercrab.data.model.ErrorKind
import com.plausiden.thundercrab.data.model.Folder

sealed interface FolderListUiState {
    data object Loading : FolderListUiState
    data class Success(val folders: List<Folder>) : FolderListUiState
    data class Error(val kind: ErrorKind, val message: String) : FolderListUiState
}
