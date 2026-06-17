//! Standalone `uniffi-bindgen` entry point for ThunderCrab.
//!
//! UniFFI 0.28 proc-macro ("library") mode reads the interface metadata baked
//! into the compiled `cdylib` rather than from a `.udl` file. The platform
//! build (Android `cargo-ndk` / iOS) drives binding generation through this
//! binary, e.g.:
//!
//! ```sh
//! cargo build -p thundercrab-ffi
//! cargo run -p thundercrab-ffi --bin uniffi-bindgen -- \
//!     generate --library target/debug/libthundercrab_ffi.so \
//!     --language kotlin --out-dir bindings/kotlin
//! ```
//!
//! Running it against the built library is also our on-host *export-surface
//! check*: if any exported type or async signature cannot cross the FFI
//! boundary, generation fails here — long before an Android toolchain is
//! involved. Requires the `cli` feature on the `uniffi` dependency.
fn main() {
    uniffi::uniffi_bindgen_main()
}
