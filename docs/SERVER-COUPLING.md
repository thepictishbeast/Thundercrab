# ThunderCrab ↔ PlausiDen Server Coupling

**Status:** Design (P2). **Author:** lead architect synthesis, 2026-06-17.
**Scope:** how client learning improves the *server-side* filter — the
flag-event → suggestions → ManageSieve → Mailroom-categories spine — plus
byte-identical-AST hardening and the JMAP path.

---

## 1. The shared spine (the "client ↔ server-Sieve ↔ ledger" axis)

The whole design hinges on **one AST shared by three consumers**:

```
  user flags/moves a message (Android)
      │  (features only — no body)
      ▼
  thundercrab-core::FlagEvent  ──record──▶  local SQLite (Db::record_flag)
      │
      ▼  Db::flag_counts_by_domain_dest()
  thundercrab-suggestions::derive_rule_candidates()   ── domain×dest dominance
      │  → DerivedRule { rule: CrabRule, local_evidence_count }
      ▼  thundercrab-suggestions::apply_suggestion()   ── safety clamp
  a SAFE CrabRule
      │
      ├──▶ local sort (thundercrab-core::eval::evaluate)         offline "learned sorting"
      ├──▶ server-side Sieve via ManageSieve  ← improves the SERVER filter (§3)
      └──▶ crab-ledger (federated corroboration; features only)  (§5)
```

The AST that travels this spine is **`CrabRule`** on the client and
**`mail_config::CategoryRule`** on the server. They are **byte-identical** (verified, §4),
which is what lets the server's Sieve emitter, the client's evaluator, and the federated
ledger all speak the same rule format.

### 1.1 Verified component status (audit)

| Component | Status |
|---|---|
| `thundercrab-core` (CrabRule, FlagEvent, Db) | REAL, tested (10 tests) |
| `thundercrab-suggestions` (`derive_rule_candidates`, `apply_suggestion`, ledger) | REAL, tested (13+4+2) |
| `crab-ledger` (Axum + 16-bit blake3 PoW gate + per-pubkey 60/hr & global 600/hr governor + 16 KiB cap) | REAL, tested (14) |
| `thundercrab-imap::managesieve::put_active_script` (connect→STARTTLS→AUTH PLAIN→PUTSCRIPT→SETACTIVE) | REAL |
| `mail_config::CategoryRules::{evaluate, to_sieve_with}` + `MessageContext` | REAL (server-only today) |
| client-side `evaluate()` | **MISSING** — port per FFI-API-SPEC §7 |

---

## 2. Flag events → suggestions (client-side, on-device)

1. **Capture (no body).** When the user moves/flags a message, the app builds a
   `FlagEvent` from the *headers it already fetched* — `from_domain_with_at`, `list_id`,
   `has_list_unsubscribe`, `subject_tokens` (lowercased, stop-word-stripped, ≤16),
   `priority_high`, plus a blake3-32 `message_hash` of the Message-Id. Full From, full
   Subject, body, recipients are **never** stored. Recorded via `record_flag_event`
   (FFI) → `Db::record_flag`.

2. **Derive.** `derive_rule_candidates(flag_counts, min_observations, dominance_threshold)`
   (defaults 3, 0.7) over `Db::flag_counts_by_domain_dest()` produces `DerivedRule`s where a
   `(domain → destination)` pair dominates ≥70% of that domain's events. Each `DerivedRule`
   carries `local_evidence_count`.

3. **Safety-clamp.** `apply_suggestion(&CrabRule)` enforces (verified): rejects
   `FileInto INBOX/Inbox/Important`, rejects `\Flagged`/`$Important`/`$Label1`, rejects
   nested actions, clamps `score` to `1..=49`, force-rewrites `origin = Federated`. A
   federated/derived rule can never promote mail into the inbox or impersonate an important
   flag.

4. **Surface, don't auto-apply (P1/P2 default).** `preview_suggestions` (FFI) shows the
   user proposed rules with their evidence count. The user accepts → `save_rule`
   (local) and/or push-to-server (§3). Auto-apply is a later, policy-gated behavior.

---

## 3. Pushing learned rules to the server-side Sieve (the coupling)

**Goal (ROADMAP P2):** confident client rules are pushed to the user's *server* Sieve via
ManageSieve, so client learning improves the server filter too — integrating with
Mailroom's deployed categories (`CategoryRules::deployed()`, the All-Mail/Junk model).

### 3.1 The clobber risk (must be designed around — do NOT ignore)

`managesieve::put_active_script` does **PUTSCRIPT + SETACTIVE and never reads back** — there
is **no GETSCRIPT, no capability/HAVESPACE parsing, PLAIN-only** (verified). A naive client
push therefore **overwrites Mailroom's server-emitted `CategoryRules` script wholesale**,
destroying the platform defaults and any other server-side rules. This is the single biggest
server-coupling hazard.

### 3.2 Three strategies, with a recommendation

| Strategy | How | Pros | Cons |
|---|---|---|---|
| **A. Server-mediated rule submission (RECOMMENDED)** | Client POSTs the safe `CrabRule` (JSON) to a Mailroom endpoint; the server merges it into its `CategoryRules` rule store and **re-emits** the Sieve via `to_sieve_with`. Client never writes Sieve directly. | Server stays the single Sieve author; no clobber; matches "client learning improves the *server* filter"; reuses the byte-identical AST end-to-end; server keeps audit/`X-PlausiDen-Category`. | Needs a new authenticated Mailroom endpoint (rule ingest). |
| B. Client merge + full re-push | Add `GETSCRIPT` to managesieve; client fetches the active script, merges its rule, re-PUTs. | No server change. | Client must parse/round-trip Sieve (fragile); two Sieve authors; merge conflicts; managesieve needs real capability parsing. |
| C. Script namespacing | Client pushes its rules to a *separate* named script; server includes it. | Isolation. | ManageSieve `include` support varies; SETACTIVE is single-script; brittle across servers. |

**Recommendation: Strategy A (server-mediated).** It keeps the server as the sole Sieve
author (no clobber), reuses the byte-identical `CrabRule`/`CategoryRule` AST as the wire
format for the ingest endpoint, and is the literal realization of "client learning improves
the server filter." The client's direct `push_sieve` FFI (FFI-API-SPEC §4.2) is retained
for the **non-PlausiDen / self-hosted** case where no Mailroom ingest endpoint exists — but
for PlausiDen mailboxes, Strategy A is the path. (Strategy B's `GETSCRIPT` + capability
parsing is a worthwhile managesieve hardening for the generic-server story, tracked
separately; it is NOT required for the PlausiDen coupling.)

### 3.3 Mailroom ingest endpoint (Strategy A, P2)

- New authenticated Mailroom route: `POST /rules` accepting one `CategoryRule` (the exact
  serde JSON of the client's `CrabRule` minus `origin`, which the server ignores).
- Server applies the **same** safety policy as `apply_suggestion` server-side (defense in
  depth), inserts into `CategoryRules`, re-emits via `to_sieve_with`, deploys to Dovecot.
- Auth reuses the mailbox credentials / an existing PlausiDen session token (decide with
  paul; do not invent a new auth scheme).

---

## 4. Integrating Mailroom categories + the "why is this here?" explainer

- **Defaults import.** `CategoryRules::deployed()` (server) is the platform baseline. The
  client imports these as `RuleOrigin::Platform` rules (score 50–99) so offline sort matches
  server behavior. Client `evaluate()` (FFI-API-SPEC §7) runs the same precedence
  (score desc, tie-break by `id`, short-circuit on `stop_on_match`) as
  `CategoryRules::evaluate`.
- **Audit header.** The server's Sieve emitter writes one `X-PlausiDen-Category` header per
  matched rule (via `editheader`). The client reads this header (it is already among the
  `other_headers` from `fetch_headers`) to power **"Why is this here?"** — showing which
  rule filed the message, with zero body access. This is a high-value, invariant-clean
  feature unlocked purely by header inspection + the shared rule IDs.

---

## 5. Federated ledger (crab-ledger)

- After local safety-clamp, a user may opt to contribute the **features only** of a derived
  rule to `crab-ledger` for federated corroboration (Ed25519-signed, canonical-JSON,
  features-only — no `message_hash`, no timestamps leave the device).
- `crab-ledger` gates writes with a 16-bit blake3 PoW + per-pubkey (60/hr) and global
  (600/hr) governor rate limits + 16 KiB body cap (verified). Corroboration is aggregated by
  distinct pubkey.
- Federated suggestions returning to a client always pass `apply_suggestion` again
  (origin → `Federated`, score clamped 1..=49, inbox/important rejected) before they can sort
  anything. Lowest trust; user-revocable.
- **Doc fix (repo discipline):** `crab-ledger/src/lib.rs` docstring omits the PoW +
  rate-limiting the code actually implements; update it so the security posture is stated
  correctly.

---

## 6. Byte-identical AST — hardening the spine (the weakest link)

**Today (verified):** `CrabRule` ≡ `mail_config::CategoryRule` — same 8 `MatchExpr` variants,
same 3 `Action` variants, same `#[serde(tag="kind", rename_all="snake_case")]`, same 6
`CategoryRule` fields (`id, display_name, when, action, score, stop_on_match`); `CrabRule`
adds only `origin` (client-only; server ignores). The invariant **holds**.

**But** the only guard is a **hand-copied JSON fixture** in
`thundercrab-suggestions/tests/schema_compat.rs` — there is **no shared crate dependency**,
so future Mailroom drift (a new `MatchExpr` variant, a renamed tag) would silently break the
spine with **zero client-side CI signal**. Two fixes, pick one:

| Option | How | Trade |
|---|---|---|
| **A. Shared no-deps crate (RECOMMENDED long-term)** | Extract `MatchExpr`/`Action`/`CategoryRule`/`CrabRule(minus origin)` into a tiny `mail-rule-ast` crate (serde-only, no deps) consumed by **both** Mailroom and ThunderCrab. | Compiler-enforced identity; impossible to drift. Requires a cross-repo dependency + coordinated release. |
| B. CI round-trip pin | A CI step in ThunderCrab pulls Mailroom's `mail-config`, serializes its canonical rules, and asserts byte-equality against `CrabRule` deserialization. | No cross-repo dep; catches drift at CI time, not compile time; needs Mailroom checkout in CI. |

**Recommendation:** B now (cheap, no coupling churn), migrate to A when both repos can take a
coordinated release — A is the only mechanism that makes drift *structurally impossible*,
which is what an "invariant" demands. Until then, the `schema_compat.rs` fixture must be
treated as load-bearing and updated lock-step with any Mailroom AST change.

---

## 7. JMAP (P2/later)

JMAP is added as a **new `Backend` impl**, not a rewrite. The `thundercrab-imap` lib
docstring already anticipates a `JmapBackend` swap-in behind the `Backend` trait. JMAP buys
efficient sync + server push (eliminating IMAP polling), and its push channel can drive the
"new mail" notification + on-device re-sort loop. Because UI/suggestions depend on the
`Backend` trait (and the FFI Object wraps a concrete backend), adding JMAP is a matter of
satisfying the trait + a second FFI constructor (`ThunderCrabClient::connect_jmap`) — no UI
or suggestion-engine changes. The no-body invariant applies identically: JMAP header/property
fetch only for the learning spine; bodies only via the walled-off display path (FFI-API-SPEC §6).

---

## 8. Verification gates

- **G-S1** `record_flag_event` → `derive_rule_candidates` → `apply_suggestion` produces only
  safe rules (existing suggestions tests stay green; add an end-to-end test through the FFI).
- **G-S2** No body bytes / full address ever reach a `FlagEvent`, `derive`, ledger payload
  (grep-gate + type-level: `FfiFlagEvent` carries only features).
- **G-S3** Byte-identical guard: the chosen option (A shared crate, or B CI round-trip)
  fails the build if a Mailroom `MatchExpr`/`Action` tag diverges.
- **G-S4** Strategy A: a `CrabRule` POSTed to Mailroom `/rules` deserializes as a
  `CategoryRule`, re-emits identical Sieve to the server's own path, and does **not** drop
  any `deployed()` default (no-clobber assertion).
- **G-S5** `evaluate_rules` (client) and `CategoryRules::evaluate` (server) return the same
  matched IDs for a shared header fixture (cross-impl parity test).
- **G-S6** Federated round-trip: a ledger-sourced suggestion re-imported client-side is
  `origin=Federated`, `score ∈ 1..=49`, never `FileInto INBOX`.
