// ============================================================================
// feature/folders/FolderListViewModel.kt  (VIEWMODEL group)
// Lists folders via the Repository. listFolders() returns Result<List<Folder>>;
// on failure the throwable is a RepositoryError carrying an ErrorKind (or a bare
// IllegalStateException("Not connected")). We never catch FfiException. Spec §1/§4.
// ============================================================================
package com.plausiden.thundercrab.feature.folders

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.plausiden.thundercrab.data.RepositoryError
import com.plausiden.thundercrab.data.ThunderCrabRepository
import com.plausiden.thundercrab.data.model.ErrorKind
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class FolderListViewModel(
    private val repo: ThunderCrabRepository,
) : ViewModel() {

    private val _uiState = MutableStateFlow<FolderListUiState>(FolderListUiState.Loading)
    val uiState: StateFlow<FolderListUiState> = _uiState.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        _uiState.value = FolderListUiState.Loading
        viewModelScope.launch {
            repo.listFolders().fold(
                onSuccess = { folders -> _uiState.value = FolderListUiState.Success(folders) },
                onFailure = { e ->
                    val kind = (e as? RepositoryError)?.kind ?: ErrorKind.UNKNOWN
                    val message = e.message ?: "Failed to load folders."
                    _uiState.value = FolderListUiState.Error(kind, message)
                },
            )
        }
    }

    companion object {
        fun factory(repo: ThunderCrabRepository): ViewModelProvider.Factory =
            viewModelFactory {
                initializer { FolderListViewModel(repo) }
            }
    }
}
