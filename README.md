> # ⚠️ DO NOT USE — UNVERIFIED — UNSAFE ⚠️
>
> This software is **unverified and unsafe for any production use**.
> It is published publicly only for transparency, third-party audit,
> and reproducibility. Treat every commit as guilty until proven
> innocent.
>
> By using this code you accept:
> - **No warranty** of any kind, express or implied.
> - **No fitness** for any particular purpose.
> - **No guarantee** of correctness, safety, or freedom from defects.
> - **Zero liability** on the maintainer for any damages — data loss,
>   security compromise, financial loss, or any consequential damages.
>
> The code is under active engineering development per the
> [Adversarial Validation Protocol v2](https://github.com/thepictishbeast/PlausiDen-AVP-Doctrine/blob/main/AVP2_PROTOCOL.md).
> Every commit's default verdict is **STILL BROKEN**. AVP-2 requires
> a minimum of 36 verification passes before a `SHIP-DECISION:`
> annotation may be considered. **No commit in this repository has
> reached `SHIP-DECISION:` status.**

# ThunderCrab

**ThunderCrab** is a privacy-first, Rust-native mail platform for the PlausiDen
mail stack. Aims to track Thunderbird's standards/protocol coverage without
inheriting its architecture.

### Editions
- **ThunderCrab** — the platform (and short name).
- **ThunderCrab for Android** — ships first (Jetpack Compose over the Rust core).
- **ThunderCrab for iOS** — Swift UI over the same core (UniFFI emits Swift).
- **ThunderCrab Desktop** — Linux desktop, reusing the same core.

One Rust core, thin native UIs over it via UniFFI (`thundercrab-ffi`). The
lowercase `thundercrab-*` crate names are the Rust packages; **ThunderCrab** is
the product brand.

> Status: foundation. Workspace compiles; schemas + safety + federated ledger
> primitives tested; IMAP/SMTP/ManageSieve backends are real; `thundercrab-ffi`
> exposes the core to native UIs (account config, connect, list folders, fetch
> headers, send, push Sieve, flag/move, rule + flag-event stores) and builds with
> tests passing. Android/iOS/desktop UIs not yet built. Governed by AVP-2 —
> nothing is `SHIP-DECISION:`.

## What makes this different

Most mail clients have static "Promotions/Social/etc." rules that don't
improve over time. ThunderCrab has a **federated rule learning** loop:

1. Every time you flag or move a message, the client logs a typed
   *flag event* — features only (sender domain, header presence,
   subject tokens), never bodies, never full addresses.
2. When patterns become consistent (e.g., "9 of 10 messages from
   `@mailchimp.com` go to Promotions"), the client **derives a rule**
   and starts applying it locally.
3. Optionally, signed copies of derived rules are submitted to a
   public corroboration ledger. Rules that N independent installs
   propose become *suggestions* surfaced in the UI for one-click
   acceptance.

Privacy invariants enforced in code (see `thundercrab-suggestions/src/safety.rs`):

- Federated rules can only **sort out of INBOX**. They cannot promote
  messages into INBOX or set `\Flagged` / `$Important`.
- The match AST has no body / full-address variants — the type system
  prevents content from ever entering a rule.
- Signing keys are per-install (Ed25519). Rotate by deleting the keypair.
- `submitted_at_day` is days-since-epoch, not minute-precision — defends
  against timing-based traffic analysis.

## Layout

```
Cargo.toml                            # workspace
thundercrab-core/                     # rule schema, flag events, SQLite store
  ├─ src/crab_rule.rs                 # CrabRule (wire-compat with mail-config)
  ├─ src/flag_event.rs                # FlagEvent record
  └─ src/db.rs                        # local SQLite layer
thundercrab-imap/                     # IMAP/SMTP/ManageSieve client (scaffold)
  └─ src/lib.rs                       # AccountConfig, Backend trait
thundercrab-suggestions/              # federated rule learning
  ├─ src/derive.rs                    # candidate-rule derivation
  ├─ src/safety.rs                    # apply_suggestion guards
  └─ src/ledger.rs                    # signed-suggestion wire format
docs/
  ├─ ARCHITECTURE.md
  ├─ THUNDERBIRD_PARITY.md
  └─ SUGGESTION_HEURISTICS.md
```

## Wire compatibility with mail-config

`thundercrab_core::CrabRule` is byte-stable with
`mail_config::CategoryRule` from `Secure-Email-Server-and-UI` modulo a
single field: `origin`. The orchestrator emits Sieve from
`CategoryRule`; ThunderCrab edits the same logical rule via ManageSieve.
The `tests/schema_compat` integration test asserts the JSON shapes match.

## Build & test

```sh
cargo test --workspace
```

## License

AGPL-3.0-or-later.
