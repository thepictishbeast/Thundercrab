//! Privacy-safe, enumerated diagnostics — the "phone-home"-safe subset.
//!
//! ThunderCrab is privacy-first. The very error this module exists to
//! help debug — `status netsol-…hostingplatform.com: Mailbox doesn't
//! exist` — is itself proof that raw error strings and folder names leak
//! PII. So the events recorded here are **enumerated**: a [`DiagKind`]
//! (what happened), a [`DiagCategory`] (coarse error family), a stable
//! numeric [`DiagEvent::code`], a `&'static str` operation label, and
//! numeric counts / durations. They carry **no** folder names, email
//! addresses, subjects, message bodies, or raw error strings — they are
//! safe to transmit off-device.
//!
//! # Two tiers of observability
//!
//! * `tracing` events (handled by the host's subscriber) keep full
//!   detail and stay in the on-device log the user controls. That is
//!   where a folder name or error string may legitimately appear,
//!   because it never leaves the device unless the user exports it.
//! * [`DiagEvent`]s recorded here are the scrubbed subset the app may
//!   *optionally* upload. Uploading is gated, in the UI layer, behind a
//!   first-run consent notice and a build flag that production builds
//!   leave off (see `docs/TELEMETRY.md`).
//!
//! The recorder is a bounded in-memory ring buffer: cheap, lock-guarded,
//! and lossy by design (the oldest events are evicted) so it can never
//! grow without bound on a long-running session.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Master switch. When `false`, [`record`] is a no-op — nothing is buffered
/// and nothing is mirrored to `tracing`'s diag target. Production builds flip
/// this off at startup so the app collects and sends nothing; debug/test
/// builds leave it on. The runtime toggle (exposed over the FFI) lets the user
/// disable telemetry from settings too. See `docs/TELEMETRY.md`.
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Enable or disable all diagnostic collection at runtime.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Whether diagnostic collection is currently enabled.
#[must_use]
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// What happened — one variant per observable operation outcome.
///
/// Fieldless (C-like) on purpose: the discriminant is part of the stable
/// [`DiagEvent::code`] wire contract documented in `docs/TELEMETRY.md`.
/// Append new variants at the end; never renumber existing ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagKind {
    /// IMAPS connect + LOGIN succeeded.
    ConnectOk = 0,
    /// IMAPS connect or LOGIN failed (see [`DiagCategory`] for the family).
    ConnectFail = 1,
    /// Folder listing succeeded with every folder fully enumerated.
    ListFoldersOk = 2,
    /// Listing succeeded but ≥1 folder was unselectable or failed STATUS
    /// and was returned with zero counts instead of aborting the load.
    ListFoldersDegraded = 3,
    /// Header fetch for a folder succeeded.
    FetchHeadersOk = 4,
    /// Header fetch failed.
    FetchHeadersFail = 5,
    /// A message move between folders succeeded.
    MoveOk = 6,
    /// A message move failed.
    MoveFail = 7,
    /// A flag set/clear succeeded.
    FlagOk = 8,
    /// A flag set/clear failed.
    FlagFail = 9,
    /// A server-side Sieve script push succeeded.
    SievePushOk = 10,
    /// A server-side Sieve script push failed.
    SievePushFail = 11,
    /// An outbound message send succeeded.
    SendOk = 12,
    /// An outbound message send failed.
    SendFail = 13,
    /// Authentication was rejected by the server.
    AuthFail = 14,
    /// A clean logout was performed.
    Logout = 15,
    /// Folder listing failed entirely (the LIST itself errored or timed
    /// out) — distinct from [`DiagKind::ListFoldersDegraded`], where the
    /// list succeeded but some folders were unusable.
    ListFoldersFail = 16,
    /// A folder mutation (create / rename / delete / subscribe) succeeded.
    FolderMutateOk = 17,
    /// A folder mutation failed.
    FolderMutateFail = 18,
}

/// Coarse error family, derived from a `BackendError` *without* its
/// string payload. [`DiagCategory::None`] means "not an error".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagCategory {
    /// Not an error — the event records a successful outcome.
    None = 0,
    /// TCP / network transport failure (unreachable host, reset, …).
    Transport = 1,
    /// Authentication or authorization failure.
    Auth = 2,
    /// Server returned a protocol-level error response.
    Protocol = 3,
    /// An operation exceeded its timeout bound.
    Timeout = 4,
    /// TLS handshake / certificate failure.
    Tls = 5,
    /// The requested feature is not implemented in this backend.
    NotImplemented = 6,
    /// An unexpected internal error (logic bug, poisoned lock, …).
    Internal = 7,
}

/// A single PII-free diagnostic record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagEvent {
    /// What happened.
    pub kind: DiagKind,
    /// Coarse error family, or [`DiagCategory::None`] for a success.
    pub category: DiagCategory,
    /// Stable numeric code: `kind * 1000 + category`. Lets a backend
    /// dashboard group/alert without parsing the enum names.
    pub code: u32,
    /// Static operation label (e.g. `"list_folders"`). Never contains
    /// user data — it is a compile-time constant.
    pub op: &'static str,
    /// Generic primary count (folders listed, headers fetched, …).
    pub count: u64,
    /// Generic secondary count (folders skipped, retries, …).
    pub extra: u64,
    /// Wall-clock duration of the operation in milliseconds, if measured.
    pub duration_ms: u64,
    /// Event time, milliseconds since the Unix epoch.
    pub at_unix_ms: u64,
}

impl DiagEvent {
    /// Construct an event, stamping `code` and `at_unix_ms` automatically.
    #[must_use]
    pub fn new(
        kind: DiagKind,
        category: DiagCategory,
        op: &'static str,
        count: u64,
        extra: u64,
        duration_ms: u64,
    ) -> Self {
        let code = (kind as u32) * 1000 + (category as u32);
        Self {
            kind,
            category,
            code,
            op,
            count,
            extra,
            duration_ms,
            at_unix_ms: now_unix_ms(),
        }
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Maximum events retained in memory. Older events are evicted FIFO.
const CAPACITY: usize = 512;

/// The process-wide diagnostic ring buffer.
///
/// `Mutex<VecDeque<_>>` is const-initializable, so no lazy/`OnceCell`
/// machinery is needed. Contention is irrelevant: events are recorded at
/// human/network timescales, not in hot loops.
static BUFFER: Mutex<VecDeque<DiagEvent>> = Mutex::new(VecDeque::new());

/// Record a pre-built [`DiagEvent`]. Also mirrors it to `tracing` at the
/// appropriate level so it shows up in the on-device verbose log.
pub fn record(ev: DiagEvent) {
    if !is_enabled() {
        return;
    }
    if ev.category == DiagCategory::None {
        tracing::debug!(op = ev.op, code = ev.code, count = ev.count, "diag");
    } else {
        tracing::warn!(
            op = ev.op,
            code = ev.code,
            category = ?ev.category,
            "diag(error)"
        );
    }
    if let Ok(mut buf) = BUFFER.lock() {
        if buf.len() >= CAPACITY {
            buf.pop_front();
        }
        buf.push_back(ev);
    }
}

/// Convenience: build and record in one call.
pub fn record_event(
    kind: DiagKind,
    category: DiagCategory,
    op: &'static str,
    count: u64,
    extra: u64,
    duration_ms: u64,
) {
    record(DiagEvent::new(kind, category, op, count, extra, duration_ms));
}

/// Copy out the current buffer without clearing it (for the UI to show
/// recent activity).
#[must_use]
pub fn snapshot() -> Vec<DiagEvent> {
    BUFFER
        .lock()
        .map(|b| b.iter().cloned().collect())
        .unwrap_or_default()
}

/// Take and clear all buffered events (for an upload that should not
/// re-send what it already shipped).
#[must_use]
pub fn drain() -> Vec<DiagEvent> {
    BUFFER
        .lock()
        .map(|mut b| b.drain(..).collect())
        .unwrap_or_default()
}

/// Discard all buffered events (e.g. when the user declines telemetry).
pub fn clear() {
    if let Ok(mut b) = BUFFER.lock() {
        b.clear();
    }
}

/// Serialize a slice of events as a JSON array (newline-free), for the
/// upload body or a log line. Infallible: returns `"[]"` on the
/// impossible serialization error.
#[must_use]
pub fn to_json(events: &[DiagEvent]) -> String {
    serde_json::to_string(events).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The recorder is process-global state; serialize the buffer-mutating
    /// tests so the default parallel runner doesn't let them clobber each
    /// other. (No `serial_test` dep — a plain mutex is enough.)
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Acquire the serialization lock and start from a clean, enabled buffer.
    fn guard() -> std::sync::MutexGuard<'static, ()> {
        let g = TEST_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        set_enabled(true);
        clear();
        g
    }

    #[test]
    fn code_is_stable_kind_times_1000_plus_category() {
        // Pure construction — touches no global state.
        let ev = DiagEvent::new(DiagKind::ConnectFail, DiagCategory::Auth, "connect", 0, 0, 12);
        // ConnectFail = 1, Auth = 2  ->  1*1000 + 2
        assert_eq!(ev.code, 1002);
    }

    #[test]
    fn ring_buffer_evicts_oldest_beyond_capacity() {
        let _g = guard();
        for _ in 0..(CAPACITY + 10) {
            record_event(DiagKind::ListFoldersOk, DiagCategory::None, "list_folders", 1, 0, 0);
        }
        assert_eq!(snapshot().len(), CAPACITY);
    }

    #[test]
    fn drain_empties_and_returns_events() {
        let _g = guard();
        record_event(DiagKind::MoveOk, DiagCategory::None, "move_message", 1, 0, 3);
        let drained = drain();
        assert_eq!(drained.len(), 1);
        assert!(snapshot().is_empty());
    }

    #[test]
    fn json_carries_no_strings_beyond_static_op() {
        let _g = guard();
        record_event(DiagKind::ListFoldersDegraded, DiagCategory::None, "list_folders", 16, 1, 50);
        let json = to_json(&snapshot());
        // op is a compile-time constant; no PII fields exist on the type
        // at all, so the JSON cannot contain a folder name or address.
        assert!(json.contains("list_folders"));
        assert!(json.contains("ListFoldersDegraded"));
    }

    #[test]
    fn disabled_recorder_buffers_nothing() {
        let _g = guard();
        set_enabled(false);
        record_event(DiagKind::ConnectOk, DiagCategory::None, "connect", 0, 0, 0);
        assert!(snapshot().is_empty());
        set_enabled(true);
    }
}
