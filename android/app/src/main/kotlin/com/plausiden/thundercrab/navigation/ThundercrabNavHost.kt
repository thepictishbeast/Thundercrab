// ============================================================================
// navigation/ThundercrabNavHost.kt  (UI group)
// The single NavHost. Wires the four screens and passes nav args + the
// Repository-backed ViewModel factories. The Repository instance is pulled from
// the app's AppContainer (ServiceLocator); ViewModels are obtained via
// viewModel(factory = ...). No Ffi* type appears here. AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.navigation

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.plausiden.thundercrab.ThundercrabApplication
import com.plausiden.thundercrab.feature.folders.FolderListScreen
import com.plausiden.thundercrab.feature.folders.FolderListViewModel
import com.plausiden.thundercrab.feature.messages.MessageListScreen
import com.plausiden.thundercrab.feature.messages.MessageListViewModel
import com.plausiden.thundercrab.feature.read.MessageReadScreen
import com.plausiden.thundercrab.feature.read.MessageReadViewModel
import com.plausiden.thundercrab.feature.setup.AccountSetupScreen
import com.plausiden.thundercrab.feature.setup.AccountSetupViewModel
import com.plausiden.thundercrab.feature.suggestions.SuggestionsScreen
import com.plausiden.thundercrab.feature.suggestions.SuggestionsViewModel

@Composable
fun ThundercrabNavHost() {
    val navController = rememberNavController()
    val context = LocalContext.current
    // The single app-lifetime Repository, via the manual ServiceLocator.
    val container = remember(context) {
        (context.applicationContext as ThundercrabApplication).container
    }
    val repository = container.repository

    NavHost(navController = navController, startDestination = Destinations.SETUP) {

        // 1. Account Setup -----------------------------------------------------
        composable(Destinations.SETUP) {
            val vm: AccountSetupViewModel =
                viewModel(factory = AccountSetupViewModel.factory(repository))
            AccountSetupScreen(
                viewModel = vm,
                onConnected = {
                    // Home is the INBOX message list (Thunderbird-style), with the
                    // mailbox list one Menu tap away — not a folder list first.
                    navController.navigate(Destinations.messages("INBOX")) {
                        popUpTo(Destinations.SETUP) { inclusive = true }
                    }
                },
            )
        }

        // 2. Folder List -------------------------------------------------------
        composable(Destinations.FOLDERS) {
            val vm: FolderListViewModel =
                viewModel(factory = FolderListViewModel.factory(repository))
            FolderListScreen(
                viewModel = vm,
                onFolderClick = { folderName ->
                    navController.navigate(Destinations.messages(folderName)) {
                        launchSingleTop = true
                    }
                },
                onOpenSuggestions = {
                    navController.navigate(Destinations.SUGGESTIONS)
                },
                onBack = { navController.popBackStack() },
            )
        }

        // 2b. Suggestions ------------------------------------------------------
        composable(Destinations.SUGGESTIONS) {
            val vm: SuggestionsViewModel =
                viewModel(factory = SuggestionsViewModel.factory(repository))
            SuggestionsScreen(
                viewModel = vm,
                onBack = { navController.popBackStack() },
            )
        }

        // 3. Message List ------------------------------------------------------
        composable(
            route = Destinations.MESSAGES_ROUTE,
            arguments = listOf(
                navArgument(Destinations.ARG_FOLDER) { type = NavType.StringType },
            ),
        ) { backStackEntry ->
            val rawFolder = backStackEntry.arguments?.getString(Destinations.ARG_FOLDER).orEmpty()
            val folder = Destinations.decodeFolder(rawFolder)
            val vm: MessageListViewModel =
                viewModel(factory = MessageListViewModel.factory(repository, folder))
            MessageListScreen(
                viewModel = vm,
                onOpenFolders = {
                    navController.navigate(Destinations.FOLDERS) {
                        launchSingleTop = true
                        popUpTo(Destinations.FOLDERS)
                    }
                },
                onMessageClick = { uid ->
                    navController.navigate(Destinations.read(folder, uid))
                },
            )
        }

        // 4. Message Read ------------------------------------------------------
        composable(
            route = Destinations.READ_ROUTE,
            arguments = listOf(
                navArgument(Destinations.ARG_FOLDER) { type = NavType.StringType },
                navArgument(Destinations.ARG_UID) { type = NavType.IntType },
            ),
        ) { backStackEntry ->
            val rawFolder = backStackEntry.arguments?.getString(Destinations.ARG_FOLDER).orEmpty()
            val folder = Destinations.decodeFolder(rawFolder)
            val uid = backStackEntry.arguments?.getInt(Destinations.ARG_UID) ?: 0
            val vm: MessageReadViewModel =
                viewModel(factory = MessageReadViewModel.factory(repository, folder, uid))
            MessageReadScreen(
                viewModel = vm,
                onBack = { navController.popBackStack() },
            )
        }
    }
}
