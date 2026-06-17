# Thundercrab Roadmap — Android-first, Rust-core, server-coupled

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

- **P0 — FFI foundation (in progress).** New `thundercrab-ffi` crate (UniFFI,
  `cdylib`+`staticlib` for Android). Exposes, at minimum: `CrabRule` evaluation,
  `AccountConfig` (incl. `plausiden()` auto-config), and async IMAP ops
  (connect, list folders, fetch headers, send, push Sieve). Generates Kotlin +
  Swift bindings. Builds on any box; bindgen runs in the Android build.
- **P1 — Android app (SHIPS FIRST).** Gradle + Jetpack Compose. Screens: account
  setup (auto-filled `mail.plausiden.com`, ports 993/587, fallback 2525/465),
  folder list, message list + read (headers/body via core), flag/move → emits a
  `FlagEvent` → suggestions. Built on a dev machine with Android SDK/NDK +
  `cargo-ndk` (NOT on plausiden-prime, which is headless).
- **P2 — Server coupling.** Flag events feed `thundercrab-suggestions`; confident
  rules are pushed to the user's **server-side Sieve via ManageSieve**, so client
  learning improves the *server* filter too (integrating with Mailroom's deployed
  categories + the All-Mail/Junk model). Add JMAP for efficient sync + push.
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
