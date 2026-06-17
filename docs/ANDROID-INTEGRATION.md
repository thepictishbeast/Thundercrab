# ThunderCrab Android Integration

**Status:** Design (P0/P1). **Author:** lead architect synthesis, 2026-06-17.
**Scope:** how the Android app is built, how it links the Rust core via UniFFI, and
the decision on *fork Thunderbird-Android's UI shell* vs *Compose-from-scratch*.

---

## 1. Decision: build Jetpack Compose from scratch over `thundercrab-ffi`

**DECISION: Compose-from-scratch. Do NOT fork the Thunderbird-Android UI shell.**

We build a greenfield Kotlin/Jetpack Compose app whose only backend is
`thundercrab-ffi` (UniFFI → Kotlin), with the integration layer modeled on
`matrix-rust-components-kotlin`. We **mine** Thunderbird/K-9 for UX patterns and a
feature checklist — we fork **none** of its code.

### 1.1 Evidence (from the analyses, verified against the upstream tree)

The "fork TB-android UI shell onto our Rust core" option assumes there is a Rust-core
integration seam in Thunderbird-Android to fork onto. There is not:

- **Thunderbird-Android ships zero native/Rust code.** The audit of
  `/tank/scratch/upstream/thunderbird-android` found **no** `.so`, `.rs`, `Cargo.toml`,
  or `CMakeLists.txt`; **no** `externalNativeBuild`, `ndkVersion`, or `cargo-ndk`
  configuration anywhere in the tree.
- The only `jniLibs` references are **packaging dedup**, not integration:
  `build-plugin/.../thunderbird.app.android.compose.gradle.kts` sets `keepDebugSymbols`
  for prebuilt AndroidX/CameraX `.so` files (`libandroidx.graphics.path.so`,
  `libdatastore_shared_counter.so`, `libimage_processing_util_jni.so`,
  `libsurface_util_jni.so`); `app-thunderbird/build.gradle.kts` and
  `app-k9mail/build.gradle.kts` use `jniLibs { excludes += 'kotlin/**' }` to strip stray
  resources. The lone "rust" hit is `[rust]` in `docs/book.toml` (an mdBook config).

So forking the shell does not graft a UI onto our Rust core — it grafts a UI onto **K-9's
Kotlin account/store/sync backend**, which is exactly the engine our Rust core replaces.
That would mean (a) two competing mail engines, (b) the no-body and zero-unsafe invariants
living in K-9's Kotlin/Java rather than our verified Rust, and (c) a direct collision with
the **AVP no-port doctrine** (do not port C++/JS — mine for features only). The TB shell's
Compose screens are welded to K-9's `MessagingController`/`Account`/`LocalStore`; the
coupling cost of unpicking them exceeds writing focused Compose screens against our
already-tested core.

### 1.2 What we mine (features, not code)

Use Thunderbird/K-9 as a *feature and UX reference only*: account-setup auto-config flow,
unified-inbox concept, swipe-to-archive/move gestures, notification grouping, folder
special-use icons, the "move to folder" sheet. None of this requires their code; it informs
our Compose screens, which call `thundercrab-ffi`.

### 1.3 Why this satisfies every constraint

| Constraint | Compose-from-scratch | Fork TB shell |
|---|---|---|
| Rust core is single source of truth | ✅ UI calls FFI only | ❌ K-9 Kotlin engine competes |
| Zero-unsafe / no-body in verified Rust | ✅ stays in core | ❌ leaks into Kotlin/Java |
| AVP no-port doctrine | ✅ mine only | ❌ adopts a JVM mail stack |
| Integration seam exists today | ✅ (UniFFI) | ❌ no native seam in TB tree |
| Effort | focused Compose screens | unpick K-9 backend coupling |

---

## 2. Module layout

```
android/
  app/                         # Compose app module
    src/main/jniLibs/<abi>/    # libuniffi_thundercrab_ffi.so per ABI (cargo-ndk output)
    src/main/java/.../uniffi/  # generated Kotlin (build/generated, added as srcDir)
    src/main/kotlin/...        # Compose screens, ViewModels (the ONLY hand-written UI)
  buildSrc / xtask             # cargo-ndk + uniffi-bindgen driver (modeled on matrix-rust-components-kotlin)
```

Template: `matrix-rust-components-kotlin` (the layer Element X ships) — cargo-ndk per ABI,
xtask-driven `uniffi-bindgen`, one `.so` per ABI under `src/main/jniLibs/<abi>/`, AAR
packaging. It is the closest production match to "tokio async Rust over UniFFI → Kotlin
coroutines." `mozilla/application-services` is the alternative reference (uses
`org.mozilla.rust-android-gradle` instead of a hand-rolled task).

---

## 3. Build pipeline (canonical recipe — one path, no UDL)

Pin to **proc-macro + library mode**. `thundercrab-ffi/src/lib.rs` already calls
`uniffi::setup_scaffolding!()`; keep the API as `#[uniffi::export]` proc-macros, **no
`.udl` file**. Drop every UDL-era step (the "rename to `libuniffi_*`", "hand-copy the
`uniffi/` folder" instructions from old tutorials are obsolete in library mode).

```bash
# Prereqs (dev machine — NOT plausiden-prime, which is headless):
#   Android SDK + NDK r27+, cargo-ndk (bbqsrc/cargo-ndk v4.x, Rust MSRV 1.86)
rustup target add aarch64-linux-android armv7-linux-androideabi \
                  x86_64-linux-android i686-linux-android

# STEP 1 — build + lay out per-ABI in one command. -o writes the jniLibs/<abi>/ layout.
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86 -t x86_64 \
  -o app/src/main/jniLibs --platform 24 build --release -p thundercrab-ffi

# STEP 2 — generate Kotlin in LIBRARY MODE from the built .so (reads name+metadata from it).
cargo run --bin uniffi-bindgen generate \
  --library app/src/main/jniLibs/arm64-v8a/libuniffi_thundercrab_ffi.so \
  --language kotlin --out-dir app/build/generated/uniffi
```

Wire both into Gradle either as (a) a hand-rolled `Exec` task with
`variant.javaCompileProvider.dependsOn(...)` + `sourceSet.java.srcDir(generatedDir)` (the
approach in UniFFI's own `gradle.html`), or (b) `org.mozilla.rust-android-gradle`.
**Recommendation:** start with the hand-rolled task (matches matrix-rust-components-kotlin,
fewer moving parts), revisit the Mozilla plugin if CI maintenance gets heavy.

### 3.1 Runtime dependencies the generated Kotlin requires

```kotlin
dependencies {
    implementation("net.java.dev.jna:jna:5.15.0@aar")            // @aar is MANDATORY (packs JNA's own .so)
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:<current>")
}
```

- **No `System.loadLibrary("...")`** — UniFFI loads the lib via JNA on first use.
- **Pin `uniffi` and `uniffi-bindgen` to the *exact* same version** (both 0.28.x). Mismatch
  produces subtly broken bindings.

### 3.2 16 KB page-size compliance (current Google Play requirement)

Build the `.so` with **NDK r27+** (16 KB-aligned by default). Pin a **16 KB-compliant JNA
`@aar`** (5.15.x+). Both your `.so` and JNA's bundled `.so` are subject to the requirement;
non-compliance is rejected at upload.

---

## 4. Async + lifecycle on the Kotlin side

- Each exported `pub async fn` becomes a Kotlin **`suspend fun`** running on the caller's
  coroutine context. Call from a `viewModelScope.launch { ... }`.
- The Rust futures use tokio (async-imap, lettre); they are made tokio-safe by
  `#[uniffi::export(async_runtime = "tokio")]` **on the `impl`** (see FFI-API-SPEC §2/§4.1).
  The FFI owns a single long-lived multi-thread `tokio::Runtime`.
- **Object lifecycle is a real burden:** `ThunderCrabClient` is an `Arc`-backed UniFFI
  Object; the JVM GC will **not** free its Rust allocation. Tie it to a `ViewModel` and call
  `.destroy()` in `onCleared()`, or wrap short-lived uses in `.use { }`.
- **Cancellation:** cancelling a Kotlin coroutine does **not** cancel the Rust future.
  Long fetches must call `client.cancel()` explicitly (exposed for this reason).
- **Errors:** `FfiError` (fielded) surfaces as a Kotlin `sealed class FfiException` with one
  subclass per variant, fields preserved — `catch (e: FfiException.Auth) { ... }`.

---

## 5. P1 screens (all call FFI; zero mail logic in Kotlin)

| Screen | FFI calls | Notes |
|---|---|---|
| Account setup | `plausiden_account_config(username)` | auto-fills host/993/587 (+2525 fallback once in `AccountConfig`); user types address + password |
| Folder list | `ThunderCrabClient.connect`, `list_folders` | special-use icons from `FfiFolder.special_use` |
| Message list | `fetch_headers(folder, limit)` | HEADER-ONLY; From/Subject/date from headers |
| Read message | `fetch_body(...)` **gated on paul** | header-only until body carve-out is ratified (FFI-API-SPEC §6); set `\Seen` explicitly via `set_flag` |
| Flag / move | `set_flag`, `move_message`, then `record_flag_event` | emits a features-only `FlagEvent` → suggestions (SERVER-COUPLING.md) |
| Why is this here? | `evaluate_rules`, read `X-PlausiDen-Category` header | offline explainer (FFI-API-SPEC §7) |
| Suggested rules | `preview_suggestions`, `save_rule` | read-only preview, user accepts → `save_rule` |
| Compose / send | `send_message(cfg, password, enc, msg)` | text/plain v0 |

---

## 6. Build matrix

| Target | Builds on plausiden-prime (headless)? | Toolchain |
|---|---|---|
| `thundercrab-ffi` (lib/cdylib/staticlib) | ✅ | stable Rust |
| `uniffi-bindgen` Kotlin generation | ✅ (needs built `.so`) | `uniffi` `cli` feature |
| Android `.so` (per-ABI) + APK/AAB | ❌ | Android SDK + NDK r27+, cargo-ndk, Gradle |

The `.so` cross-build and the app build require a dev machine with the Android SDK/NDK.
The FFI crate itself (lib) and the bindgen step build anywhere, including prime.

---

## 7. Verification gates

- **G-A1** `cargo ndk … build --release -p thundercrab-ffi` produces 4 `.so` files under
  `app/src/main/jniLibs/<abi>/`.
- **G-A2** `uniffi-bindgen generate --library …` produces Kotlin that compiles against
  jna `@aar` + coroutines.
- **G-A3** `readelf`/`alignment-check` confirms 16 KB alignment on every `.so` (ours + JNA's).
- **G-A4** App launches in an emulator; account-setup screen auto-fills from
  `plausiden_account_config`.
- **G-A5** Connect → list_folders → fetch_headers round-trips against a live PlausiDen test
  mailbox; no body bytes appear in any FlagEvent/suggestion path (grep-gate from FFI spec).
- **G-A6** A `ThunderCrabClient` bound to a `ViewModel` is `.destroy()`d on `onCleared`
  (leak check under repeated rotation).
