# Thundercrab Architecture

## One-paragraph summary

Thundercrab is a Rust mail client whose differentiator is *learned*
sorting that improves both locally and federated-ly without ever
seeing message content. The same `CrabRule` AST is what the
orchestrator compiles to Sieve server-side, what the client evaluates
offline, and what the federated ledger corroborates between users.

## Data flow

```
                              ┌────────────────────────────────────┐
                              │         FEDERATED LEDGER           │
                              │  (separate service; out of scope    │
                              │   of this repo. Accepts             │
                              │   SignedSuggestion, returns         │
                              │   corroboration counts.)            │
                              └──────────────▲──────┬──────────────┘
                                             │      │
                                       sign+ │      │ pull suggestions
                                       submit│      │ above N corroborators
                                             │      ▼
   user flags/moves                  ┌──────────────────────┐
   message in UI         FlagEvent   │   thundercrab-       │
   ───────────────►      ──────────► │   suggestions        │ ─── derive ───►
                                     │                      │     candidate rules
                                     └─────────▲────────────┘
                                               │
                                  read events  │  read flags counts
                                               │
                                     ┌──────────────────────┐
                                     │   thundercrab-core   │
                                     │   (SQLite, schema,   │
                                     │   FlagEvent, CrabRule)│
                                     └─────────▲────────────┘
                                               │
                                               │ rules from ManageSieve;
                                               │ messages from IMAP
                                               │
                                     ┌──────────────────────┐
                                     │   thundercrab-imap   │
                                     │   (Backend trait,    │
                                     │    IMAP/SMTP/Sieve)  │
                                     └─────────▲────────────┘
                                               │
                                  ┌────────────┴────────────┐
                                  │                          │
                       PlausiDen mail server         any IMAP server
                       (mail.plausiden.com)          (Fastmail, etc.)
                       runs Sieve compiled
                       from mail-config::
                       CategoryRules
```

## Crate boundaries

| Crate | Responsibility | What it does NOT do |
|-------|----------------|---------------------|
| `thundercrab-core` | Wire schema + local SQLite | No network, no UI, no derivation logic |
| `thundercrab-imap` | Backend trait + concrete IMAP/SMTP/Sieve impls | No rule logic — just message movement |
| `thundercrab-suggestions` | Derive rules; safety-check; sign; verify | No persistence (uses `core` for that) |

A future GUI crate (`thundercrab-gui`?) consumes `Backend` from
`thundercrab-imap` and `Db` + suggestions from the other two.

## Wire-compat with `mail-config`

`mail_config::CategoryRule` (server-side) and `thundercrab_core::CrabRule`
(client-side) are deliberately the same shape:

| Field | mail-config | thundercrab-core | Notes |
|-------|-------------|------------------|-------|
| `id` | string | string | identical |
| `display_name` | string | string | identical |
| `when` | `MatchExpr` | `MatchExpr` | identical AST, identical serde tags |
| `action` | `Action` | `Action` | identical AST, identical serde tags |
| `score` | i32 | i32 | identical |
| `stop_on_match` | bool | bool | identical |
| `origin` | (absent) | `RuleOrigin` | client-only; server ignores |

A round-trip test in `thundercrab-suggestions/tests/schema_compat.rs`
asserts a `CategoryRule` JSON deserializes into a `CrabRule` cleanly
when `origin` is supplied externally.

## Federated learning — invariants

These are NOT advisory; they are enforced in code by
`thundercrab_suggestions::safety::apply_suggestion`:

1. **No promote into INBOX/Important.** A suggestion whose action is
   `FileInto { folder: "INBOX" | "Inbox" | "Important" }` is rejected
   at safety-check time. Promotions to importance are user-only.
2. **No flag-setting on protected flags.** `\Flagged`, `$Important`,
   `$Label1` cannot be touched by federated rules.
3. **Score clamp.** Federated rules are clamped to `[1, 49]` so they
   never outrank platform defaults (50–99) or user rules (100+).
4. **Origin is rewritten.** Even if the wire blob says `origin: "user"`,
   `apply_suggestion` rewrites it to `Federated` — the local DB never
   stores a federated rule under any other origin.

## Threat model (non-exhaustive)

| Attack | Mitigation |
|--------|------------|
| Adversary submits "promote sketchy.com → INBOX" | `safety::apply_suggestion` rejects on protected folder |
| Adversary submits "label everything Important" | `apply_suggestion` rejects on protected flag |
| Adversary forges a rule under another user's key | Ed25519 signature over canonical body |
| Adversary submits the same rule under many keys (Sybil) | Ledger-side rate-limiting + reputation (out of repo scope) |
| Network observer correlates submission timing | `submitted_at_day` is day-precision; submit-batching client-side |
| Submitter accidentally leaks message content via rule text | Rule AST has no body/full-address variants — type system blocks it |

## Where this is going

- `thundercrab-imap` gets a real IMAP4rev2 backend (probably built on
  `async-imap`) and a JMAP backend.
- A `thundercrab-gui` crate (toolkit TBD: egui / iced / dioxus-desktop).
- A small "Why is this in Promotions?" panel that reads the audit
  `X-PlausiDen-Category` header (server-side) AND replays
  `evaluate(...)` locally to explain client-side rules.
- Federated ledger as a separate service repo. Out of scope here.
