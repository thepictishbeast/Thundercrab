# ThunderCrab FFI API Specification

**Status:** Design (P0). **Author:** lead architect synthesis, 2026-06-17.
**Crate:** `thundercrab-ffi` (UniFFI 0.28, `cdylib`+`staticlib`+`lib`).
**Audience:** the Android (Kotlin/Compose) app; later the Linux desktop UI.

This spec defines the *exact* UniFFI surface the Android app calls, mapped
one-to-one onto **real, verified** `thundercrab-imap` / `thundercrab-core`
signatures. The audit confirmed the FFI is currently a 3-symbol stub
(`thundercrab_version`, `AccountSettings`, `plausiden_account`) that does **not**
even depend on `thundercrab-imap`. This is the headline P0 work item.

---

## 0. Invariants this surface MUST preserve (AVP doctrine)

1. **Rust-native.** No C++/JS ported. We mine Thunderbird/K-9 for *features*, not code.
2. **Zero `unsafe`.** Workspace already sets `unsafe_code = "forbid"`; FFI inherits it.
3. **No code path reads message bodies** for the learning/ledger spine. The P0
   surface is **header-only**. `FlagEvent` is features-only (verified: `message_hash`,
   `from_domain_with_at`, `list_id`, `has_list_unsubscribe`, `subject_tokens`,
   `priority_high` — no body, no full address). See §6 for the body-display carve-out.
4. **`thundercrab-ffi` is a thin, logic-free pass-through.** Every exported function is a
   trivial forwarder to `thundercrab-core` / `thundercrab-imap`. If logic appears here, that
   is the bug. The current `plausiden_account()` *violates* this (re-hardcodes ports +
   a 2525 fallback that `AccountConfig::plausiden()` lacks); §5 fixes it.

---

## 1. Cargo.toml deltas (P0 prerequisite)

The current `thundercrab-ffi/Cargo.toml` depends only on `thundercrab-core`. It must add:

```toml
[dependencies]
uniffi = { version = "0.28", features = ["tokio", "cli"] }   # tokio: async export; cli: uniffi-bindgen
thundercrab-core        = { path = "../thundercrab-core" }
thundercrab-imap        = { path = "../thundercrab-imap" }
thundercrab-suggestions = { path = "../thundercrab-suggestions" }   # for the suggestion preview (read-only)
tokio = { version = "1", features = ["rt-multi-thread", "sync"] }

[[bin]]                  # library-mode bindgen entrypoint (UniFFI 0.28 modern path, no .udl)
name = "uniffi-bindgen"
path = "src/bin/uniffi-bindgen.rs"
```

`src/bin/uniffi-bindgen.rs` is the canonical one-liner: `fn main() { uniffi::uniffi_bindgen_main() }`.

---

## 2. Bridging constraints (from the UniFFI analysis — these shape the API)

- **`Backend` trait is NOT dyn-compatible.** It uses RPITIT (`-> impl Future + Send`),
  so it cannot be boxed or exported as a UniFFI interface. The bridge is a **concrete
  exported Object** (`#[derive(uniffi::Object)]`) wrapping a `RustImapBackend`.
- **`async_runtime = "tokio"` goes on the `impl` block, not behind a trait** (uniffi-rs
  #2576: ineffective on exported async-trait impls → "no reactor" panics). All
  tokio-touching async methods live on the concrete Object's `impl`.
- **Own one long-lived multi-thread tokio `Runtime`** (lazy static / `Arc`) for the app
  lifetime. Document the Kotlin side: the exported Object holds a Rust allocation the JVM
  GC will **not** free — every long-lived object must be `.destroy()`d (tie to
  `ViewModel.onCleared()`), or use `.use{}`.
- **Only owned values cross the FFI.** No lifetimes, no generics, no borrowed slices.
  `&[&str]` → `Vec<String>`; `MessageContext<'a>` → an owned `OwnedMessageContext`.
- **No cross-FFI cancellation.** Long-running async ops expose an explicit `cancel()`.
- **Errors:** the FFI error enum derives `#[derive(uniffi::Error)]` + `thiserror`
  (fielded, NOT `flat_error`) so Kotlin gets a `sealed class FfiException` with one
  subclass per variant and fields preserved.

---

## 3. Exported value types (UniFFI Records — owned copies)

These mirror the real `thundercrab-imap` / `thundercrab-core` types as `#[derive(uniffi::Record)]`
pass-through copies. The FFI converts at the boundary; it adds no logic.

```rust
/// Mirrors thundercrab_imap::AccountConfig (verified fields).
#[derive(uniffi::Record, Clone)]
pub struct FfiAccountConfig {
    pub imap_host: String,
    pub imap_port: u16,      // 993
    pub smtp_host: String,
    pub smtp_port: u16,      // 587 (STARTTLS) | 465 (implicit)
    pub sieve_port: u16,     // 4190
    pub username: String,
}

/// Mirrors thundercrab_imap::FolderSummary (verified).
#[derive(uniffi::Record, Clone)]
pub struct FfiFolder {
    pub name: String,
    pub special_use: Option<String>,
    pub messages: u64,
    pub unseen: u64,
}

/// Mirrors thundercrab_imap::MessageHeaders (verified). HEADER-ONLY.
#[derive(uniffi::Record, Clone)]
pub struct FfiHeaders {
    pub uid: u32,
    pub folder: String,
    pub from: String,                          // raw From: line
    pub subject: String,                       // raw Subject: line
    pub other_headers: Vec<FfiHeaderPair>,     // List-Id, List-Unsubscribe, X-Priority, ...
}
#[derive(uniffi::Record, Clone)]
pub struct FfiHeaderPair { pub name: String, pub value: String }

/// Maps SmtpEncryption (verified enum).
#[derive(uniffi::Enum, Clone, Copy)]
pub enum FfiSmtpEncryption { StartTls, ImplicitTls }

/// Owned send request. Built into smtp::OutboundMessage<'a> inside the forwarder.
#[derive(uniffi::Record, Clone)]
pub struct FfiOutboundMessage {
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,        // text/plain only (v0), per OutboundMessage docstring
}

/// Owned mirror of CrabRule (thundercrab_core). Byte-identical AST to Mailroom — see SERVER-COUPLING.md.
#[derive(uniffi::Record, Clone)]
pub struct FfiCrabRule {
    pub id: String,
    pub display_name: String,
    pub when_json: String,   // serde_json of MatchExpr (preserves byte-identical tags across FFI)
    pub action_json: String, // serde_json of Action
    pub score: i32,
    pub stop_on_match: bool,
    pub origin: String,      // "user" | "platform" | "federated"
}

/// Owned, body-free view for offline rule evaluation (mirror of MessageContext<'a>, owned).
#[derive(uniffi::Record, Clone)]
pub struct FfiMessageContext {
    pub headers: Vec<FfiHeaderPair>,  // lowercased keys, original-case values
    pub from_address: String,         // full, lowercased; empty if unparseable
    pub subject: String,
}
```

> **Why `when_json`/`action_json` as strings instead of recursive UniFFI enums:** the
> byte-identical spine requires the *exact* serde representation to survive client↔server.
> Carrying the canonical JSON across the FFI and deserializing inside the forwarder
> guarantees the wire bytes are produced by `thundercrab-core`'s own serde, not by a
> hand-rolled Kotlin re-serializer that could drift. The Kotlin side treats them as opaque
> blobs handed back to the FFI verbatim.

```rust
/// Fielded error → Kotlin `sealed class FfiException`.
#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum FfiError {
    #[error("transport: {msg}")] Transport { msg: String },
    #[error("auth: {msg}")]      Auth { msg: String },
    #[error("protocol: {msg}")]  Protocol { msg: String },
    #[error("not implemented: {what}")] NotImplemented { what: String },
    #[error("invalid input: {msg}")]    InvalidInput { msg: String },  // e.g. bad rule JSON
}
// From<thundercrab_imap::BackendError> for FfiError is the only mapping logic allowed here.
```

---

## 4. Exported functions & objects — the 8 capabilities

The task lists eight capabilities. Each maps to a **verified** real signature. **Critical
mappings the audit forces (do NOT auto-map to the `Backend` trait):**
`put_sieve` on the trait returns `NotImplemented` by design — push-Sieve goes to the free
fn `managesieve::put_active_script`; and `send` is the free fn `smtp::send_message`. Both
need the **password per call** and Sieve needs a **script_name** the trait method never takes.

| # | Capability | FFI symbol | Real backend signature (verified) |
|---|---|---|---|
| 1 | account config | `plausiden_account_config(username) -> FfiAccountConfig` | `AccountConfig::plausiden(host, username)` |
| 2 | connect | `ThunderCrabClient::connect(cfg, password) -> Arc<ThunderCrabClient>` | `RustImapBackend::connect(&AccountConfig, &str)` (async) |
| 3 | list folders | `client.list_folders() -> Vec<FfiFolder>` | `Backend::list_folders(&self)` |
| 4 | fetch headers | `client.fetch_headers(folder, limit) -> Vec<FfiHeaders>` | `Backend::fetch_headers(&self, &str, Option<u32>)` |
| 5 | fetch body | `client.fetch_body(...)` | **DOES NOT EXIST** — see §6, gated on paul |
| 6 | send | `send_message(cfg, password, enc, msg)` | `smtp::send_message(&AccountConfig,&str,SmtpEncryption,&OutboundMessage)` (free fn) |
| 7 | push Sieve | `push_sieve(cfg, password, script_name, script)` | `managesieve::put_active_script(&AccountConfig,&str,&str,&str)` (free fn) |
| 8 | record flag-event | `client.flag_and_record(...)` + `record_flag_event(db_path, ev)` | `Backend::set_flag`/`move_message` + `Db::record_flag` |

Plus supporting: offline rule evaluation, suggestion preview, version.

### 4.1 The connection Object (stateful: connect/list/fetch/move/flag)

```rust
#[derive(uniffi::Object)]
pub struct ThunderCrabClient {
    inner: RustImapBackend,   // session-bound; password consumed+dropped on connect (verified)
    cancel: AtomicBool,
}

// CAPABILITY 2 — connect. Free async fn, NOT a #[uniffi::constructor].
// Two reasons: (a) UniFFI 0.28.x async-constructor support is not relied upon
// (this is the matrix-rust-sdk pattern for the same reason); (b) it returns a
// Result, which a constructor cannot. Maps to RustImapBackend::connect (async).
#[uniffi::export(async_runtime = "tokio")]
pub async fn connect(cfg: FfiAccountConfig, password: String)
    -> Result<Arc<ThunderCrabClient>, FfiError>;

#[uniffi::export(async_runtime = "tokio")]   // on the impl, NOT a trait (#2576)
impl ThunderCrabClient {
    /// CAPABILITY 3 — list folders. Maps to Backend::list_folders.
    pub async fn list_folders(&self) -> Result<Vec<FfiFolder>, FfiError>;

    /// CAPABILITY 4 — fetch headers (HEADER-ONLY; BODY.PEEK[HEADER]). Maps to Backend::fetch_headers.
    pub async fn fetch_headers(&self, folder: String, limit: Option<u32>)
        -> Result<Vec<FfiHeaders>, FfiError>;

    /// CAPABILITY 8 (part 1) — move a message. Maps to Backend::move_message.
    pub async fn move_message(&self, from: String, to: String, uid: u32) -> Result<(), FfiError>;

    /// CAPABILITY 8 (part 2) — set/clear a flag. Maps to Backend::set_flag.
    pub async fn set_flag(&self, folder: String, uid: u32, flag: String, set: bool)
        -> Result<(), FfiError>;

    /// No cross-FFI cancellation in UniFFI → explicit cancel for long fetches.
    pub fn cancel(&self);

    /// Clean logout. Best-effort; maps to RustImapBackend::logout SEMANTICS.
    pub async fn logout(&self);
}
```

> **Signature-fidelity note (load-bearing):** the real `RustImapBackend::logout(self)`
> **consumes `self`** (it does `self.session.into_inner()`). A UniFFI Object method is
> `&self` on an `Arc<ThunderCrabClient>`, so you *cannot* move `inner` out to call the
> consuming method. Resolution: change the Object's field to
> `inner: tokio::sync::Mutex<Option<RustImapBackend>>` and `logout`/destroy does `.take()`
> then calls the real consuming `logout`. (`RustImapBackend` already holds its session in a
> `Mutex`, so this is the only structural change required.) Do NOT write `logout(&self)`
> that forwards to a consuming method — it will not compile.

> Lifecycle note for Kotlin: `ThunderCrabClient` is an `Arc`-backed Object. The JVM GC
> will **not** free it. Bind it to a `ViewModel`, call `.destroy()` in `onCleared()`.

### 4.2 Stateless free functions (send / push-Sieve / config / eval)

These take `cfg + password` per call (no retained password — honors `connect()`'s
consume-and-drop discipline). They open and close their own transport.

```rust
/// CAPABILITY 1 — account config. Pure pass-through to AccountConfig::plausiden.
#[uniffi::export]
pub fn plausiden_account_config(username: String) -> FfiAccountConfig;

/// CAPABILITY 6 — send. Builds smtp::OutboundMessage internally (owned→borrowed at the seam).
#[uniffi::export(async_runtime = "tokio")]
pub async fn send_message(
    cfg: FfiAccountConfig,
    password: String,
    encryption: FfiSmtpEncryption,
    message: FfiOutboundMessage,
) -> Result<(), FfiError>;

/// CAPABILITY 7 — push Sieve. Maps to managesieve::put_active_script (NOT Backend::put_sieve).
/// `script_name` is required by ManageSieve and is NOT on the trait method.
/// SERVER-COUPLING.md governs WHAT script is pushed and the clobber-avoidance strategy.
#[uniffi::export(async_runtime = "tokio")]
pub async fn push_sieve(
    cfg: FfiAccountConfig,
    password: String,
    script_name: String,
    script: String,
) -> Result<(), FfiError>;

/// Offline rule evaluation. Mirrors mail_config::CategoryRules::evaluate (ported into core — see §7).
/// Returns the matched rule IDs in precedence order. HEADER-ONLY context. Powers "why is this here?".
#[uniffi::export]
pub fn evaluate_rules(rules: Vec<FfiCrabRule>, ctx: FfiMessageContext) -> Vec<String>;

/// Read-only suggestion preview (no apply). Wraps thundercrab_suggestions::derive_rule_candidates
/// + apply_suggestion safety clamp, surfaced for the UI's "suggested rules" screen.
#[uniffi::export]
pub fn preview_suggestions(db_path: String, min_obs: i64, dominance: f64)
    -> Result<Vec<FfiCrabRule>, FfiError>;

#[uniffi::export]
pub fn thundercrab_version() -> String;
```

### 4.3 Local ledger DB ops (flag recording + rules)

These wrap `thundercrab_core::Db` (synchronous, SQLite). Exposed as free functions over a
`db_path` so the Kotlin side owns the file location (app-private storage).

```rust
/// CAPABILITY 8 (part 3) — persist the features-only FlagEvent. Maps to Db::record_flag.
/// The FfiFlagEvent carries ONLY features (no body, no full address) — enforced by type.
#[uniffi::export]
pub fn record_flag_event(db_path: String, ev: FfiFlagEvent) -> Result<(), FfiError>;

#[uniffi::export] pub fn save_rule(db_path: String, rule: FfiCrabRule) -> Result<(), FfiError>;   // Db::save_rule
#[uniffi::export] pub fn load_rules(db_path: String) -> Result<Vec<FfiCrabRule>, FfiError>;        // Db::load_rules
#[uniffi::export] pub fn delete_rule(db_path: String, id: String) -> Result<bool, FfiError>;        // Db::delete_rule
```

```rust
/// Owned mirror of thundercrab_core::FlagEvent. FEATURES ONLY — by construction.
#[derive(uniffi::Record, Clone)]
pub struct FfiFlagEvent {
    pub message_hash: Vec<u8>,        // 32 bytes; validated len==32 → InvalidInput otherwise
    pub source: String,               // "manual_move" | "toggle_flag" | "explicit_category" | "accept_suggestion"
    pub destination: String,
    pub from_domain_with_at: String,  // "@github.com"
    pub list_id: Option<String>,
    pub has_list_unsubscribe: bool,
    pub subject_tokens: Vec<String>,  // already lowercased+stop-word-stripped by caller
    pub priority_high: bool,
    pub observed_at_unix_ms: i64,     // converted to chrono DateTime<Utc> in the forwarder
}
```

---

## 5. Fixing the thin-pass-through violation (P0, must land with the bridge)

`plausiden_account()` currently re-hardcodes `mail.plausiden.com`/993/587/STARTTLS + a
**2525 fallback**, while `AccountConfig::plausiden()` has **no 2525**. The two auto-configs
have already drifted. Fix:

1. `plausiden_account_config(username)` **delegates** to `AccountConfig::plausiden(host, username)`
   and converts — no inline port literals in the FFI.
2. **Resolve 2525**: the single source of truth is `AccountConfig`. Add an explicit
   `smtp_fallback_port: Option<u16>` to `AccountConfig` (P0, one-line additive change in
   `thundercrab-imap`) so the server-mirrored config carries the carrier fallback, OR drop
   2525 from the FFI entirely. **Recommendation:** add it to `AccountConfig` — the fallback
   is real PlausiDen submission behavior and belongs with the other ports, not duplicated
   in the binding layer. Keep the existing `AccountSettings`/`plausiden_account` symbols as
   deprecated shims for one release to avoid breaking any early Kotlin scaffolding.

---

## 6. `fetch_body` — display-only carve-out, GATED ON PAUL (do not build in P0)

The capability list names "fetch body," but:
- `thundercrab-imap` has **no body-fetch method** (only `BODY.PEEK[HEADER]`; verified).
- There is **no MIME parser** in the workspace (THUNDERBIRD_PARITY marks MIME-body as not done).
- A mail app must render bodies, yet the no-body invariant + ledger spine forbid bodies
  *touching the learning path*.

**Resolution (recommended, requires paul's explicit ratification before P1 build):**
the no-body invariant is **scoped to the learning/ledger spine**, which already holds
absolutely (FlagEvent is features-only; suggestions/ledger never see a body). Body fetch
for **display only** is a *separate*, walled-off path:

- New `thundercrab-imap` method `fetch_body(folder, uid) -> RenderedBody` using `BODY[]`
  (note: NOT `PEEK` would set `\Seen`; use `BODY.PEEK[]` to avoid silent read-marking, then
  `set_flag(\Seen)` explicitly from the UI) + a `mail-parser` dependency for MIME.
- A distinct FFI Object method `client.fetch_body(...)` returning text + sanitized HTML.
- **Compile-time wall:** the body path lives in its own module with a doc/lint barrier; no
  type from it may be referenced by `flag_event.rs`, `thundercrab-suggestions`, or
  `crab-ledger`. A grep-gate in CI asserts `mail-parser`/`fetch_body` symbols never appear
  in the suggestions/ledger crates.

**Until paul ratifies the scoping, the P0 FFI surface is header-only** and `fetch_body`
returns `NotImplemented`. This keeps us from shipping anything that could violate the
invariant before the decision lands. (AVP memory: flag for paul, do not decide unilaterally.)

---

## 7. Porting `evaluate` into core (allowed: mining for a feature, not porting code)

`thundercrab-core` has **no `evaluate()`** — the client cannot sort offline today, and
`db.rs`'s doc-link to `mail_config::evaluate` is dangling. Port the evaluator semantics
(NOT a foreign codebase — this is the same-author Rust spine):

- Add `thundercrab_core::eval::evaluate(rules: &[CrabRule], ctx: &MessageContext) -> Vec<&CrabRule>`
  mirroring `mail_config::CategoryRules::evaluate` (verified: sort by `score` desc, tie-break
  by `id`, short-circuit on `stop_on_match`).
- Add an owned `MessageContext` (the Mailroom one is `MessageContext<'a>`; FFI needs owned).
- Mirror `eval_match` over the 8 MatchExpr variants (all header/Subject/From-domain only).
- This is the engine behind `evaluate_rules()` (§4.2) and the "why is this here?" explainer
  that reads the server's `X-PlausiDen-Category` audit header (see SERVER-COUPLING.md).

---

## 8. Build pipeline (full detail in ANDROID-INTEGRATION.md)

```
# 1. per-ABI .so straight into jniLibs (cargo-ndk, -o writes the layout)
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86 -t x86_64 \
  -o app/src/main/jniLibs --platform 24 build --release
# 2. Kotlin bindings in library mode (read metadata from the built .so — no .udl, no rename)
cargo run --bin uniffi-bindgen generate \
  --library app/src/main/jniLibs/arm64-v8a/libuniffi_thundercrab_ffi.so \
  --language kotlin --out-dir app/build/generated/uniffi
```

Runtime deps the generated Kotlin needs: `net.java.dev.jna:jna:5.15.x@aar` (the `@aar`
classifier is mandatory) + `org.jetbrains.kotlinx:kotlinx-coroutines-core`. NDK r27+ for
16 KB page alignment. **No `System.loadLibrary` call** — UniFFI loads via JNA on first use.

---

## 9. Verification gates for this spec

- `cargo build -p thundercrab-ffi` clean with the new deps; `cargo test -p thundercrab-ffi` green.
- `unsafe` count across workspace stays 0 (`unsafe_code = "forbid"` holds).
- A test asserts `plausiden_account_config(u)` equals a conversion of
  `AccountConfig::plausiden("mail.plausiden.com", u)` — drift can never recur.
- A grep-gate asserts no body/MIME symbol leaks into `flag_event`/suggestions/ledger.
- `uniffi-bindgen generate --library …` produces Kotlin that compiles against the deps in §8.
- Round-trip test: `FfiCrabRule.when_json` → core `MatchExpr` → re-serialize is byte-identical.
