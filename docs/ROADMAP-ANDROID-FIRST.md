# ThunderCrab Roadmap — Android-first, Rust-core, server-coupled

**Decision (2026-06-17).** One Rust core, thin native UIs over it. Ship order:
**Android first → Linux desktop → deepen PlausiDen-server coupling.** The Iced
GUI (`thundercrab-gui`) is desktop/winit-only and cannot run on Android, so it is
*not* the path to mobile. Instead, the existing tested Rust crates become a
single engine exposed to platform UIs via **UniFFI**.

## Architecture

```
   thundercrab-core      (CrabRule AST, flag events, db)   ── shared, tested
   thundercrab-imap      (IMAPS / SMTP / ManageSieve)       ── shared, real
   thundercrab-suggestions + crab-ledger (federated learning)
        │
        ▼
   thundercrab-ffi  (NEW)  — UniFFI surface: rule eval, account config,
        │                    connect/list/fetch/send/push-sieve, flag events
        ├── Android app (Kotlin + Jetpack Compose)   ← SHIPS FIRST
        └── Linux desktop (reuse core; Iced or egui) ← SECOND
```

Why this shape: the core is the single source of truth and is the same code the
server compiles to Sieve (`mail_config::CategoryRule` ≡ `CrabRule`). UIs hold no
logic — they call the FFI. This is the Signal/Firefox pattern (Rust core +
per-platform UI) and keeps the zero-unsafe / no-body-access invariants in one place.

## Phases

- **P0 — FFI foundation. ✅ DONE.** `thundercrab-ffi` (UniFFI proc-macro mode,
  `lib`+`cdylib`+`staticlib`) exposes `CrabRule`/`AccountConfig` (incl. `plausiden()`
  auto-config), async IMAP ops (connect/list/fetch/move/setFlag/logout, send, push
  Sieve), flag-event + rule stores, `previewSuggestions`, and `rulesToSieve`. In-tree
  `uniffi-bindgen` emits Kotlin + Swift; export-surface verified.
- **P1 — Android app. ✅ BUILT + HEADLESS-VERIFIED** (correction: Android APKs build
  headless — done **on plausiden-prime**, no dev machine needed). Jetpack Compose,
  five screens (account setup auto-filled `mail.plausiden.com` 993/587/4190; folder
  list; message list + headers-only read; flag/move → `FlagEvent`; Suggestions).
  `assembleDebug` (all 4 ABIs) + signed universal APK + `assembleRelease` (R8) green;
  launches to a resumed activity; `FfiOnDeviceTest` 2/2 on emulator; `RuleSummaryTest`
  7/7. Not yet: human-driven UX pass, real release-key signing.
- **P2 — Server coupling. ◐ IN PROGRESS (offline-safe parts done).** Flag events feed
  `thundercrab-suggestions`; `thundercrab-core::sieve` emits a personal Sieve
  (`sievec`-validated) that *layers on top of* Mailroom's global `sieve_before` (not a
  byte-clone). Suggestions UI surfaces derive → accept → emitted-Sieve preview. The
  end-to-end chain (events → derive → gate → emit → sievec) is proven offline
  (`pipeline_e2e`). **GATED:** the live **ManageSieve push** is deliberately not wired
  — needs explicit operator consent + a throwaway test account; never a real mailbox
  unattended. Still ahead: JMAP for efficient sync + push.
- **P3 — Linux desktop.** Reuse the same core/FFI; finish/replace the Iced GUI to
  reach Android feature parity.
- **P4 — "cooler things."** Live `crab-ledger` federated corroboration, on-device
  rule learning, push notifications, the spam-report/learn loop mirrored from the
  server, end-to-end encrypted draft sync.

## Build matrix
| Target | Builds on prime? | Toolchain |
|---|---|---|
| core / imap / ffi (lib) | ✅ | stable Rust |
| Android `.so` + app | ❌ (needs SDK/NDK) | `cargo-ndk`, Android Studio/Gradle |
| Linux desktop GUI | ✅ (with display deps) | `cargo build -p thundercrab-gui` |

## Governance
AVP-2 applies: nothing here is `SHIP-DECISION:`. UI code is unverified by default.
Invariants that must survive every phase: **zero `unsafe`**, **no code path reads
message bodies** (headers/Subject/From-domain only), FFI is a thin pass-through.
