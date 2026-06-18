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

# JNA ships optional desktop integration (Native$AWT.getWindowID) that references
# java.awt.*/javax.swing.* — JDK-only classes absent on Android. They are never
# reached at runtime, but R8 fails the build on the dangling references unless we
# tell it to ignore them. Standard JNA-on-Android suppression.
-dontwarn java.awt.**
-dontwarn javax.swing.**
-dontwarn com.sun.jna.**
