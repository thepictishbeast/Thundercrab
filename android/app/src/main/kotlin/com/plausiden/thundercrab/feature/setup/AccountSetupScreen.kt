// ============================================================================
// feature/setup/AccountSetupScreen.kt  (UI group)
// Screen 1 — Account Setup. User enters full email + password; host/ports are
// PREFILLED from the core (repo.accountDraftFor via the VM) and shown read-only,
// never hardcoded in Kotlin. Connect submits; on success the VM flips
// connected=true and this screen calls onConnected() to navigate to "folders".
//
// Governance (spec §5.2): the password lives only in the VM's transient UiState
// and is passed straight into the connect call. It is never persisted here.
// AVP-2: UNVERIFIED.
// ============================================================================
package com.plausiden.thundercrab.feature.setup

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.plausiden.thundercrab.data.model.AccountDraft

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AccountSetupScreen(
    viewModel: AccountSetupViewModel,
    onConnected: () -> Unit,
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }

    // Navigate once the VM reports a live connection.
    LaunchedEffect(state.connected) {
        if (state.connected) onConnected()
    }

    // Surface any connect error once, then tell the VM to clear it.
    LaunchedEffect(state.errorMessage) {
        val msg = state.errorMessage
        if (msg != null) {
            val kind = state.errorKind?.name ?: "ERROR"
            snackbarHostState.showSnackbar("$kind: $msg")
            viewModel.consumeError()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Add account") },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant,
                ),
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                text = "Connect your mailbox",
                style = MaterialTheme.typography.headlineMedium,
            )
            Text(
                text = "Enter your email address and password. Server details are " +
                    "filled in automatically and your password is never stored on this device.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            OutlinedTextField(
                value = state.email,
                onValueChange = viewModel::onEmailChanged,
                label = { Text("Email address") },
                singleLine = true,
                enabled = !state.connecting,
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Email,
                    imeAction = ImeAction.Next,
                ),
                modifier = Modifier.fillMaxWidth(),
            )

            OutlinedTextField(
                value = state.password,
                onValueChange = viewModel::onPasswordChanged,
                label = { Text("Password") },
                singleLine = true,
                enabled = !state.connecting,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Password,
                    imeAction = ImeAction.Done,
                ),
                modifier = Modifier.fillMaxWidth(),
            )

            // Prefilled, read-only server details from the core.
            val draft = state.draft
            if (draft != null) {
                ServerDetailsCard(draft)
            }

            Spacer(modifier = Modifier.height(4.dp))

            val canConnect = state.draft != null &&
                state.email.isNotBlank() &&
                state.password.isNotBlank() &&
                !state.connecting
            Button(
                onClick = viewModel::onConnect,
                enabled = canConnect,
                modifier = Modifier.fillMaxWidth(),
            ) {
                if (state.connecting) {
                    CircularProgressIndicator(
                        modifier = Modifier.height(20.dp).width(20.dp),
                        strokeWidth = 2.dp,
                        color = MaterialTheme.colorScheme.onPrimary,
                    )
                    Spacer(modifier = Modifier.width(12.dp))
                    Text("Connecting…")
                } else {
                    Text("Connect")
                }
            }
        }
    }
}

@Composable
private fun ServerDetailsCard(draft: AccountDraft) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            text = "Server details",
            style = MaterialTheme.typography.titleMedium,
        )
        DetailRow("IMAP", "${draft.imapHost}:${draft.imapPort}")
        DetailRow("SMTP", "${draft.smtpHost}:${draft.smtpPort}")
        DetailRow("Sieve", "${draft.imapHost}:${draft.sievePort}")
        DetailRow("Username", draft.username)
    }
}

@Composable
private fun DetailRow(label: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}
