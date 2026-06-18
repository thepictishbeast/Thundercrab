# ThunderCrab for Android

> **AVP-2 — UNVERIFIED — UNSAFE — nothing here is `SHIP-DECISION:`.**
> The app now **builds headless** (debug + R8 release), **signs** (debug key),
> **launches** to a resumed activity, and passes an **on-device FFI test** + JVM
> unit tests — see **Verification status** below. It has **not** been exercised as a
> human-driven UX, is **not** signed with a real release key, and the live
> ManageSieve push is **deliberately disabled**. Treat every claim as provisional
> until independently checked.

A thin Jetpack Compose front-end for the ThunderCrab mail engine. **All mail logic
lives in the Rust core** (`thundercrab-core` / `thundercrab-imap` / `thundercrab-suggestions`),
exposed to Kotlin through `thundercrab-ffi` over [UniFFI](https://mozilla.github.io/uniffi-rs/).
The app holds **no business logic** — rules, sorting, suggestion derivation, and Sieve
compilation all happen in Rust. The Kotlin side only calls the FFI and shape-translates
at a single seam (the Repository). This is the Signal / Firefox pattern: a verified Rust
core with a per-platform native UI.

The app is built greenfield in Compose — it does **not** fork the Thunderbird-Android /
K-9 UI shell (that shell is welded to K-9's own Kotlin mail engine, which our Rust core
replaces). We mine Thunderbird/K-9 for UX patterns only, never code.

- **Package:** `com.plausiden.thundercrab`
- **Single Gradle module:** `:app`
- **Generated UniFFI Kotlin package:** `uniffi.thundercrab_ffi`
- **Native library soname:** `libthundercrab_ffi.so` (crate `thundercrab-ffi`)

---

## What's in this build (P1 — five screens; P2 — server coupling)

| # | Screen | Route | FFI it calls (via the Repository) |
|---|--------|-------|-----------------------------------|
| 1 | Account Setup | `setup` | `plausidenAccountConfig(username)` to prefill host/ports; `connect(cfg, password)` on submit |
| 2 | Folder List | `folders` | `listFolders()`; top-bar action opens Suggestions |
| 3 | Message List | `messages/{folder}` | `fetchHeaders(folder, 50)`; row actions `setFlag(...)` / `moveMessage(...)` each followed by `recordFlagEvent(...)` |
| 4 | Message Read | `read/{folder}/{uid}` | **none for the body** — headers come from an in-memory cache populated by screen 3 |
| 5 | Suggestions | `suggestions` | `previewSuggestions(minObs,dominance)` → accept = `saveRule`; `loadRules` / `deleteRule`; `rulesToSieve(...)` shows the emitted personal Sieve **read-only** |

**P2 server coupling (offline-safe).** Screen 5 closes the federated-learning loop on
the phone: flag/move events (screen 3) → derived suggestions → one-tap accept → the
exact personal Sieve that *would* install, shown read-only. The Rust core emits the
Sieve (`thundercrab-core::sieve`, validated against Dovecot's `sievec`). **The live
ManageSieve push (`pushSieve`) is deliberately NOT wired** — the "Push to server" button
is a disabled placeholder. A live push requires explicit operator consent and a throwaway
test account; it will never target a real mailbox unattended. `sendMessage` is also not
yet surfaced.

---

## Verification status (headless, on prime)

Proven (not assumed) by the toolchain on plausiden-prime:

| What | How | Status |
|------|-----|--------|
| Rust core cross-compiles to Android | `cargo ndk … -p thundercrab-ffi` | ✅ all 4 ABIs |
| App compiles vs real generated bindings | `./gradlew :app:assembleDebug` | ✅ |
| Signed universal debug APK | `apksigner verify` | ✅ |
| Launches to a resumed `MainActivity` | headless emulator (android-34 x86_64) | ✅ no crash |
| Rust core loads + runs through JNA on-device | instrumented `FfiOnDeviceTest` | ✅ 2/2 |
| Rule-summary logic | JVM unit tests `RuleSummaryTest` | ✅ 7/7 |
| Sieve emitter | Dovecot `sievec` | ✅ (in `thundercrab-core`) |
| Release / R8 minified build | `./gradlew :app:assembleRelease` | ✅ (JNA AWT `-dontwarn`) |

**Not yet done:** a human-driven UX pass on a real device; the live ManageSieve push;
release-key signing. AVP-2: nothing is `SHIP-DECISION:`.

---

## Governing invariants (must survive into the app)

### No message-body access — anywhere
The FFI `fetchBody` is a hard `NotImplemented` stub by design (it always throws
`FfiException.NotImplemented`). The whole IMAP surface is header-only by construction
(`BODY.PEEK[HEADER]`); **no message body is ever read.** Consequences enforced here:

- The Repository interface **does not expose `fetchBody` at all** — there is no caller.
- The Message Read screen renders a fixed, explicitly **disabled** body card with the
  literal text *"Message body not available in this build."* It does **not** call
  `fetchBody` and never surfaces a body-fetch error.
- Review gate: the string `fetchBody` appears in **zero** hand-written files under
  `app/src/main/kotlin/` — it exists only in generated UniFFI code.

### No stored secrets
The account password lives **only** in the Account Setup ViewModel's transient in-memory
state and in the `connect(draft, password)` FFI call (where the Rust core consumes and
drops it after LOGIN). It is **never** written to `SharedPreferences`, DataStore, or any
file. There is **no credential persistence at all** in this scaffold — relaunching the
app returns to the `setup` screen. (Encrypted credential storage is a later phase.) The
on-device SQLite store (`thundercrab.db`) holds only rules and features-only flag events —
no bodies, no passwords.

### No business logic in Kotlin
Rules/sorting/suggestions/Sieve all live in Rust. The one place the "no logic" rule
visibly bends is constructing a features-only flag event from headers inside the
Repository (documented there as the single bent-rule seam); the long-term fix is to push
event construction into the FFI core.

---

## Version matrix (pinned — do not drift)

| Component | Version |
|---|---|
| JDK | 21 (Temurin) |
| compileSdk | 34 |
| targetSdk | 34 |
| minSdk | 26 |
| Android Gradle Plugin (AGP) | 8.5.2 |
| Gradle | 8.9 |
| Kotlin | 2.0.21 (+ `org.jetbrains.kotlin.plugin.compose`, same version) |
| Compose BOM | 2024.09.00 (Material3) |
| NDK | 26.3.11579264 |
| JNA | `net.java.dev.jna:jna:5.14.0@aar` |
| Coroutines | `org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1` |
| Lifecycle (viewmodel-compose) | `androidx.lifecycle:lifecycle-viewmodel-compose:2.8.4` |
| Activity Compose | `androidx.activity:activity-compose:1.9.1` |
| Navigation Compose | `androidx.navigation:navigation-compose:2.8.0` |
| Core KTX | `androidx.core:core-ktx:1.13.1` |

These are locked in `gradle/libs.versions.toml`, `app/build.gradle.kts`, and
`gradle/wrapper/gradle-wrapper.properties`. Kotlin 2.0 supplies the Compose compiler via
the `kotlin.plugin.compose` Gradle plugin (locked to the Kotlin version) — there is **no**
`composeOptions { kotlinCompilerExtensionVersion = ... }` block.

---

## Prerequisites

Builds **must run on a developer machine with the Android SDK + NDK and a Rust
toolchain** — not on a headless server. The Rust crate itself (`thundercrab-ffi` lib) and
the UniFFI bindgen step build anywhere, but the Android `.so` cross-build and the APK/AAB
require the SDK/NDK.

1. **JDK 21 (Temurin).** Point `JAVA_HOME` at it (or let the AGP Java toolchain resolve 21).
2. **Android SDK** with `compileSdk` 34 platform + build-tools, and **NDK `26.3.11579264`**:
   ```bash
   export ANDROID_SDK_ROOT="$HOME/Android/Sdk"     # adjust to your install
   export ANDROID_HOME="$ANDROID_SDK_ROOT"          # legacy alias some tools still read
   sdkmanager "platforms;android-34" "build-tools;34.0.0" "ndk;26.3.11579264"
   ```
   Gradle resolves the NDK by the pinned version, so it must be installed under
   `$ANDROID_SDK_ROOT/ndk/26.3.11579264`.
3. **Rust + cargo-ndk + the four Android targets:**
   ```bash
   cargo install cargo-ndk
   rustup target add aarch64-linux-android armv7-linux-androideabi \
                     x86_64-linux-android i686-linux-android
   ```
   (ABIs map: `arm64-v8a`→`aarch64`, `armeabi-v7a`→`armv7`, `x86_64`→`x86_64`, `x86`→`i686`.)
4. **`local.properties`** (or `ANDROID_SDK_ROOT`/`ANDROID_HOME`) so Gradle can find the SDK.
   `local.properties` is git-ignored:
   ```properties
   sdk.dir=/home/you/Android/Sdk
   ```

---

## Generate the Gradle wrapper

`gradle/wrapper/gradle-wrapper.jar` is a binary that cannot be hand-authored as text. If
it is missing or you want to regenerate the wrapper, run a **system Gradle** once to emit
it pinned to 8.9:

```bash
gradle wrapper --gradle-version 8.9
```

This writes/refreshes `gradlew`, `gradlew.bat`, `gradle-wrapper.jar`, and
`gradle-wrapper.properties`. After that, always invoke the project via `./gradlew` so the
exact pinned Gradle (8.9) is used.

---

## Build

```bash
./gradlew assembleDebug
```

That single command does everything, because the UniFFI native-build and binding-gen
steps are wired into the build graph (see next section). The debug APK lands at:

```
app/build/outputs/apk/debug/app-debug.apk
```

It is signed with the auto-generated debug keystore (`~/.android/debug.keystore`,
password `android`) — **a debug key, never a release key.** The `release` build type in
this scaffold reuses the debug signing config purely so it assembles; it is **not** a
shippable, properly-signed release (AVP-2).

---

## How the cargo-ndk + uniffi-bindgen wiring works

There is **no UniFFI Gradle plugin** and **no `.udl` file** — the crate uses proc-macro /
library mode (`uniffi::setup_scaffolding!()` in `thundercrab-ffi/src/lib.rs`). Two manual
`Exec` tasks in `app/build.gradle.kts` drive the Rust side, and the Android build depends
on them, so `./gradlew assembleDebug` triggers the whole chain automatically:

1. **`cargoNdkBuildThundercrab`** — cross-compiles the Rust FFI crate to one `.so` per ABI
   and lays them out under `app/src/main/jniLibs/<abi>/`. Equivalent command (run for you):
   ```bash
   cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -t x86 -p 26 \
     -o app/src/main/jniLibs build --release -p thundercrab-ffi --lib
   ```
   (`-p 26` matches `minSdk`; `-p thundercrab-ffi --lib` are load-bearing — they build the
   FFI library and nothing else.)

2. **`generateUniffiBindings`** (depends on the build above) — runs the crate's bundled
   `uniffi-bindgen` binary in **library mode** against the arm64 `.so` (it reads the name
   and metadata from the library), emitting Kotlin into
   `app/build/generated/source/uniffi/java/uniffi/thundercrab_ffi/`. Equivalent command:
   ```bash
   cargo run -p thundercrab-ffi --bin uniffi-bindgen -- generate \
     --library app/src/main/jniLibs/arm64-v8a/libthundercrab_ffi.so \
     --language kotlin --out-dir app/build/generated/source/uniffi/java
   ```

3. **Hookup** — that generated directory is added to the `main` source set, every
   `KotlinCompile` task `dependsOn(generateUniffiBindings)`, and `preBuild` depends on the
   cargo-ndk build so the `.so` files are present before AGP packages them.

Both `app/src/main/jniLibs/` (cargo-ndk output) and `app/build/generated/...` (bindgen
output) are **build artifacts**, not checked-in source — they are git-ignored and
regenerated on each build.

The generated Kotlin loads the native library through JNA on first use (there is **no**
`System.loadLibrary` call), which is why `net.java.dev.jna:jna:5.14.0@aar` is a mandatory
dependency consumed as `@aar` — the plain desktop JAR omits JNA's own bundled `.so` and
fails at runtime on a device.

---

## Sideload the debug APK onto a phone

1. On the phone: enable **Developer options → USB debugging** (and allow the RSA prompt
   when you plug in).
2. From the dev machine:
   ```bash
   adb devices                                              # confirm the phone is listed
   adb install -r app/build/outputs/apk/debug/app-debug.apk # -r reinstalls over an existing copy
   ```
3. No USB / no `adb`? Copy `app-debug.apk` to the phone (USB file transfer, email to
   yourself, etc.), then open it with a file manager and allow **install from unknown
   sources** for that app when prompted.

On launch the app opens the **Account Setup** screen, prefilled from
`plausidenAccountConfig` (host + IMAP/SMTP/Sieve ports come from the core, never hardcoded
in Kotlin). Enter the full email address and password to connect.

---

## Known risks (AVP-2 — documented, not fixed)

- **NDK r26 vs 16 KB page size.** The matrix pins `26.3.11579264` (r26), which is **not**
  16 KB-aligned by default (r27+ would be). The pin wins, so we keep r26 and mitigate by
  building our `.so` with `RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"` (set by the
  `cargoNdkBuildThundercrab` task). Separately, JNA 5.14.0's **own bundled `.so`** is a
  distinct Google-Play 16 KB-compliance item to verify before any store submission. Not a
  build error today.
- **Stripped release `.so` metadata.** If `uniffi-bindgen generate --library` ever fails on
  the release lib, fall back to a debug build of the FFI crate and point `--library` at
  `target/debug/libthundercrab_ffi.so` (one-line swap, no restructure).
- **Gradle daemon environment.** The cargo task sets `ANDROID_NDK_HOME` from
  `android.ndkDirectory`. If the daemon cannot find `cargo` / `cargo-ndk`, prepend
  `~/.cargo/bin` to `PATH` in the task (a commented line is provided in `app/build.gradle.kts`).
- **Config cache off.** The manual UniFFI `Exec` tasks read `android.ndkDirectory` at
  configuration time and are not config-cache-safe as written, so the Gradle configuration
  cache is intentionally disabled for the scaffold (follow-up item).
- **Client lifecycle.** The single `ThunderCrabClient` is owned for the app's lifetime by
  the Repository and freed on explicit logout/close (and reclaimed on process death). It is
  not eagerly destroyed on every config change. A production build should add an
  app-lifecycle observer that `close()`s the handle on terminal exit.

---

## Stale doc warning

The repo's older `docs/ANDROID-INTEGRATION.md` describes an **aspirational** surface that
disagrees with what is actually built. Where they conflict, this README and the real
`thundercrab-ffi` code are authoritative. In particular the stale doc is wrong about:
the soname (`libuniffi_thundercrab_ffi.so` → really `libthundercrab_ffi.so`), the NDK
(`r27+` → pinned r26), JNA (`5.15.0` → `5.14.0`), an `evaluate_rules` / "Why is this here?"
screen and a Suggested-rules screen (not in P1), a `client.cancel()` call (does not exist),
and `fetch_body` being "gated on paul" (it is a hard `NotImplemented` stub; the read screen
is a fixed disabled state and never calls it).
