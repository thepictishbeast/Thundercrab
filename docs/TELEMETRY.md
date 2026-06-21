# ThunderCrab Telemetry & Diagnostics

**Single source of truth for everything ThunderCrab collects, logs, or sends.**
If a field is collected and it is not listed here, that is a bug — fix the code
or fix this doc.

ThunderCrab is privacy-first (AGPL, "no message bodies" core invariant). The bug
that motivated this system — `status netsol-…hostingplatform.com: Mailbox doesn't
exist` — is itself proof that **raw error strings and folder names leak PII**. So
telemetry is built around enumerated codes, never free text.

---

## 1. Two tiers

| Tier | Where it lives | Detail level | Leaves the device? |
|------|----------------|--------------|--------------------|
| **Verbose log** | `tracing` → on-device rotating log file | Full (may include folder names, error strings) | **No** — unless the user manually exports/shares it |
| **Diagnostics events** | `thundercrab_core::telemetry` ring buffer | **Scrubbed, enumerated** (no PII) | Only via explicit, opt-in upload |

The verbose log is the human-debug surface the *user controls*. The diagnostics
events are the only thing that may ever be transmitted, and they are PII-free *by
construction* — the `DiagEvent` type has no field that can hold user data.

---

## 2. What a diagnostics event contains (the ENTIRE schema)

`thundercrab_core::telemetry::DiagEvent` / FFI `FfiDiagEvent`:

| Field | Type | Example | Notes |
|-------|------|---------|-------|
| `kind` | enum name | `ConnectFail` | what happened (§2.1) |
| `category` | enum name | `Auth` | coarse error family (§2.2) |
| `code` | u32 | `1002` | `kind*1000 + category`, stable wire code |
| `op` | static str | `list_folders` | **compile-time constant**, never user data |
| `count` | u64 | `16` | primary count (folders listed, headers fetched) |
| `extra` | u64 | `1` | secondary count (folders skipped, retries) |
| `duration_ms` | u64 | `204` | wall-clock of the op |
| `at_unix_ms` | u64 | `1718712345678` | event time |

**That is the complete list.** There is no field for: message bodies, subjects,
sender/recipient addresses, folder names, passwords, error strings, hostnames,
IPs, device IDs, or account identifiers. None exist on the type.

### 2.1 `DiagKind`
`ConnectOk(0)`, `ConnectFail(1)`, `ListFoldersOk(2)`, `ListFoldersDegraded(3)`,
`FetchHeadersOk(4)`, `FetchHeadersFail(5)`, `MoveOk(6)`, `MoveFail(7)`,
`FlagOk(8)`, `FlagFail(9)`, `SievePushOk(10)`, `SievePushFail(11)`, `SendOk(12)`,
`SendFail(13)`, `AuthFail(14)`, `Logout(15)`, `ListFoldersFail(16)`.

> Discriminants are a stable wire contract: **append new variants at the end,
> never renumber.**

### 2.2 `DiagCategory`
`None(0)` (success), `Transport(1)`, `Auth(2)`, `Protocol(3)`, `Timeout(4)`,
`Tls(5)`, `NotImplemented(6)`, `Internal(7)`.

Derived from the error **variant only** (`ffi_category`), plus our own static
detail prefixes (`"…timed out"`, `"tls …"`) for Timeout/Tls refinement — never
from user-supplied text.

---

## 3. How collection is controlled (disable in production)

Three independent gates, any of which stops collection/transmission:

1. **Master runtime switch** — `telemetry_set_enabled(bool)` (FFI) flips an
   atomic in core. When off, `record()` is a no-op: nothing is buffered, nothing
   is mirrored to `tracing`. **The app calls `telemetry_set_enabled(BuildConfig.TELEMETRY_ENABLED)`
   at startup; production release builds ship `TELEMETRY_ENABLED = false`.**
2. **User setting** — Settings → "Send diagnostics" toggles the same switch and
   persists the choice. Declining also calls `diagnostics_clear()`.
3. **First-run consent** — on first launch the app shows a notice ("This build
   sends anonymous diagnostics to help debugging") and does not upload until the
   user accepts. (See §5.)

The in-memory ring buffer (§4) is harmless even when uploads are off — it powers
the in-app **Diagnostics** screen so a user can *see* and manually share what
would be sent. With the master switch off, the buffer stays empty.

---

## 4. Retention

- **Device, in-memory:** bounded ring buffer, `CAPACITY = 512` events, FIFO
  eviction. Lost on process exit. Never grows without bound.
- **Device, on-disk verbose log:** rotating file (size-capped, N rotations),
  app-private storage. Cleared on uninstall.
- **Server (if/when an endpoint is deployed):** see §5 — retention is set at the
  endpoint and documented there before it goes live.

`diagnostics_drain()` empties the buffer (used by an upload so it never re-sends);
`diagnostics_clear()` discards without returning (used on opt-out).

---

## 5. Transmission ("phone home") — status: DEPLOYED (debug builds)

Owner-approved and live as of 2026-06-21.

**Channel (as built):**
- `POST https://dev.plausiden.com/thundercrab/diag` (valid TLS).
- `Authorization: Bearer <token>` — debug-build-only; **write-only** (the token
  only permits appending to a server log; nothing is readable through it).
- Body = `diagnostics_json()` (a JSON array of the §2 events, nothing else).
- Server side: `thundercrab-diag-ingest` — a tiny `axum` service
  (`/home/paul/projects/thundercrab-diag-ingest`) running as a hardened systemd
  unit on `127.0.0.1:8899` (DynamicUser, ProtectSystem=strict, NoNewPrivileges).
  Validates the bearer token, caps the body at 256 KiB, and **appends one JSON
  line** to `/var/log/thundercrab-diag/diag.log`. Caddy reverse-proxies
  `/thundercrab/diag` → it (rewriting the path to `/diag`).
- Client: `repo.uploadDiagnostics()` flushes on app background (`onStop`),
  best-effort, no-op when telemetry is off, clears the buffer on success.

**Gates still honored:** off when the master switch is off; first-run consent;
production builds don't carry the token. Server log retention: rotate/prune
manually for now (low volume). Verified end-to-end: `204` with token, `401`
without, events landing in the log.

---

## 6. Auditing what would be sent

`diagnostics_json()` returns the exact bytes an upload would send, so the
Diagnostics screen can show the user "here is precisely what we transmit." Tests
(`thundercrab-core/src/telemetry.rs`) assert the JSON contains only the static
`op` label and enum names — never PII.
