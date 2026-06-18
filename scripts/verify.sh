#!/usr/bin/env bash
# ThunderCrab verification ladder — the reproducible "is it green" gate.
#
#   ./scripts/verify.sh           fast:  cargo workspace tests (+ sievec where present)
#                                        + Android debug build (arm64) + JVM unit tests
#   ./scripts/verify.sh --full    above + R8 release build + on-device instrumented
#                                        tests (needs a running emulator/device)
#
# Requirements: stable Rust + cargo-ndk + the 4 android targets, JDK 17+, Android
# SDK/NDK. On plausiden-prime, source the user-space toolchain env first:
#   . /tank/scratch/android-toolchain/env.sh
# (sets JAVA_HOME / ANDROID_SDK_ROOT / NDK / TMPDIR off the noexec /tmp). The
# cargo `sievec_validation` test self-skips when Dovecot's sievec is absent.
#
# AVP-2: a green run is NOT a SHIP-DECISION; it is the baseline gate, nothing more.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
FULL=0
if [ "${1:-}" = "--full" ]; then FULL=1; fi
GRADLE_ARGS="--no-daemon"

step() { printf '\n== %s ==\n' "$1"; }

step "Rust workspace tests (incl. sieve emitter + sievec + pipeline e2e)"
# thundercrab-gui (Iced/winit) is desktop-only and does NOT compile on a headless
# host ("platform not supported by winit" — needs X11/Wayland). It's P3 desktop
# work; build/test it on a desktop. Everything else (the Android-relevant core)
# must pass here.
cargo test --workspace --exclude thundercrab-gui

step "Android :app:assembleDebug (arm64-v8a)"
( cd "$ROOT/android" && ./gradlew :app:assembleDebug -Ptc.abis=arm64-v8a $GRADLE_ARGS )

step "Android JVM unit tests"
( cd "$ROOT/android" && ./gradlew :app:testDebugUnitTest -Ptc.abis=arm64-v8a $GRADLE_ARGS )

if [ "$FULL" = 1 ]; then
  step "Android :app:assembleRelease (R8 minify)"
  ( cd "$ROOT/android" && ./gradlew :app:assembleRelease -Ptc.abis=arm64-v8a $GRADLE_ARGS )

  step "Android instrumented tests (connectedDebugAndroidTest — needs emulator/device)"
  ( cd "$ROOT/android" && ./gradlew :app:connectedDebugAndroidTest -Ptc.abis=x86_64 $GRADLE_ARGS )
fi

printf '\n== ALL GREEN ==\n'
