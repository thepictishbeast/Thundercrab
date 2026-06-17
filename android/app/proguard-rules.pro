# ThunderCrab Android — ProGuard/R8 rules (BUILD group). AVP-2: UNVERIFIED — UNSAFE.
#
# UniFFI's generated Kotlin maps Rust types onto JNA Structure subclasses that
# are accessed reflectively across the FFI boundary. R8 must NOT rename/strip
# those members or the bindings break at runtime.

# JNA core — reflective access to native structures/callbacks.
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { *; }

# Generated UniFFI surface for thundercrab-ffi.
-keep class uniffi.thundercrab_ffi.** { *; }
