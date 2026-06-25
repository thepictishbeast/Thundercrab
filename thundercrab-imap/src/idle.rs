//! IMAP IDLE (RFC 2177) push watcher on a dedicated connection.
//!
//! Real-time mail arrival without polling: the watcher opens its *own*
//! IMAPS connection (IMAP is single-channel — the command [`Backend`]
//! must stay free for the UI), `SELECT`s a folder, and parks in `IDLE`
//! until the server reports activity or a re-arm timeout elapses.
//!
//! [`Backend`]: crate::Backend
//!
//! ## Usage shape
//!
//! The watcher is consumed and returned by each [`IdleWatcher::wait`] so
//! the caller drives a simple loop — typically from an Android foreground
//! service that holds a Keystore-encrypted password for re-auth:
//!
//! (Non-compiled illustration — the compiled, runnable version lives in
//! `examples/idle_watch.rs`. Workspace convention is `text` doc snippets so
//! `missing_docs = "deny"` + merged doctests don't choke on a synthetic crate.)
//!
//! ```text
//! let mut watcher = IdleWatcher::connect(cfg, password, "INBOX").await?;
//! loop {
//!     let (event, next) = watcher.wait_rearm().await?;
//!     if event == IdleEvent::Activity {
//!         // re-fetch headers, post a notification, etc.
//!     }
//!     watcher = next; // resume watching (re-arms IDLE)
//! }
//! ```
//!
//! ## Re-arm, not keep-alive forever
//!
//! RFC 2177 lets a server drop a silent `IDLE` after ~30 minutes. The
//! watcher therefore returns control on a [`IDLE_REARM`] timeout even
//! with no activity, so the caller can issue `DONE` + `IDLE` again and
//! keep the connection authoritative. A dropped TCP connection surfaces
//! as a [`BackendError`] on the next call so the service can reconnect.

use std::time::Duration;

use async_imap::extensions::idle::IdleResponse;

use crate::rust_imap::{ImapSession, connect_session};
use crate::{AccountConfig, BackendError};

/// Re-arm interval for a quiet `IDLE`.
///
/// RFC 2177 warns that servers MAY terminate `IDLE` after 30 minutes of
/// inactivity. We re-arm at 29 minutes — matching `async-imap`'s own
/// default and staying safely under that ceiling with a minute of margin.
pub const IDLE_REARM: Duration = Duration::from_secs(29 * 60);

/// The result of one [`IdleWatcher::wait`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleEvent {
    /// The server reported activity on the folder (new message, flag
    /// change, expunge). The caller should re-fetch and then resume
    /// watching.
    Activity,
    /// The re-arm timeout elapsed with no server activity. Nothing to do
    /// but resume watching; the round-trip keeps the connection fresh.
    Idle,
}

/// A dedicated IMAPS connection parked in `IDLE` on a single folder.
///
/// Separate from [`crate::rust_imap::RustImapBackend`] by design: IMAP is
/// single-channel, so a connection sitting in `IDLE` cannot also serve
/// the UI's `LIST` / `FETCH` / `MOVE`. The watcher owns its own session.
pub struct IdleWatcher {
    session: ImapSession,
    folder: String,
}

impl IdleWatcher {
    /// Connect, authenticate, and `SELECT` `folder` (e.g. `"INBOX"`),
    /// leaving the session ready to enter `IDLE`.
    ///
    /// `password` is used only for `LOGIN` (via [`connect_session`]) and
    /// is not retained.
    ///
    /// # Errors
    /// - [`BackendError::Transport`] / [`BackendError::Auth`] from the
    ///   connect path.
    /// - [`BackendError::Protocol`] if `SELECT folder` fails.
    pub async fn connect(
        config: &AccountConfig,
        password: &str,
        folder: &str,
    ) -> Result<Self, BackendError> {
        let mut session = connect_session(config, password).await?;
        session
            .select(folder)
            .await
            .map_err(|e| BackendError::Protocol(format!("select {folder}: {e}")))?;
        Ok(Self {
            session,
            folder: folder.to_string(),
        })
    }

    /// The folder this watcher is parked on.
    #[must_use]
    pub fn folder(&self) -> &str {
        &self.folder
    }

    /// Enter `IDLE` and block until the server reports activity or
    /// `timeout` elapses, whichever comes first.
    ///
    /// Consumes and returns `self` so the caller loops without
    /// re-establishing the connection: `let (event, w) = w.wait(d).await?;`.
    ///
    /// # Errors
    /// [`BackendError::Protocol`] if the `IDLE` / `DONE` handshake fails
    /// (most often because the underlying connection dropped — the caller
    /// should reconnect via [`IdleWatcher::connect`]).
    pub async fn wait(self, timeout: Duration) -> Result<(IdleEvent, Self), BackendError> {
        let Self { session, folder } = self;

        let mut handle = session.idle();
        handle
            .init()
            .await
            .map_err(|e| BackendError::Protocol(format!("idle init on {folder}: {e}")))?;

        // `wait_with_timeout` hands back a future that borrows `handle`
        // mutably; scope it so that borrow is released before `done()`
        // takes `handle` by value.
        let response = {
            let (wait_fut, _interrupt) = handle.wait_with_timeout(timeout);
            wait_fut
                .await
                .map_err(|e| BackendError::Protocol(format!("idle wait on {folder}: {e}")))?
        };

        let session = handle
            .done()
            .await
            .map_err(|e| BackendError::Protocol(format!("idle done on {folder}: {e}")))?;

        let event = match response {
            IdleResponse::NewData(_) => IdleEvent::Activity,
            // We never trigger the manual interrupt, but if one ever
            // arrives, treat it like a timeout (re-arm) rather than
            // inventing a third caller-visible state.
            IdleResponse::Timeout | IdleResponse::ManualInterrupt => IdleEvent::Idle,
        };

        Ok((event, Self { session, folder }))
    }

    /// [`IdleWatcher::wait`] using the RFC-2177-safe [`IDLE_REARM`] interval.
    ///
    /// # Errors
    /// As [`IdleWatcher::wait`].
    pub async fn wait_rearm(self) -> Result<(IdleEvent, Self), BackendError> {
        self.wait(IDLE_REARM).await
    }

    /// Log out cleanly. Best-effort — a failed logout is logged and the
    /// session dropped anyway.
    pub async fn logout(mut self) {
        if let Err(e) = self.session.logout().await {
            tracing::warn!(error = %e, folder = %self.folder, "idle watcher logout failed; dropping session");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rearm_interval_respects_rfc2177_ceiling() {
        // RFC 2177: a server MAY terminate IDLE after 30 minutes of
        // inactivity. Re-arming must happen strictly before that, with
        // margin, and not so eagerly that we thrash the connection.
        assert!(
            IDLE_REARM < Duration::from_secs(30 * 60),
            "re-arm must beat the 30-minute IDLE ceiling"
        );
        assert!(
            IDLE_REARM >= Duration::from_secs(20 * 60),
            "re-arm should not thrash the connection"
        );
    }

    #[test]
    fn idle_event_is_copy_and_comparable() {
        // The loop pattern (`if event == IdleEvent::Activity`) relies on
        // these derives; lock them in.
        let e = IdleEvent::Activity;
        let f = e;
        assert_eq!(e, f);
        assert_ne!(IdleEvent::Activity, IdleEvent::Idle);
    }
}
