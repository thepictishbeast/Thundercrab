# Thunderbird Parity Tracker

This file tracks the **standards and capabilities** Thunderbird supports
versus what ThunderCrab implements. This is the honest version of "track
Thunderbird improvements" — Thunderbird is ~3M LOC of C++/JS that cannot
be auto-ported to Rust. Instead we treat Thunderbird's release notes as
a feature backlog: when they ship a new protocol or auth mechanism, we
add a line here.

## How to update this doc

When a Thunderbird release adds a relevant feature:

1. Find the entry below or add a new row.
2. Note the Thunderbird version and link to release notes.
3. File an issue tagged `parity` if ThunderCrab does not yet support it.

When this repo ships support for a feature, flip ❌ → ✅ and link the PR.

## Last reviewed

- **Thunderbird release reviewed:** 128 ESR (latest as of 2026-04).
  Re-review every minor release.

## Protocols & data formats

| Feature | Thunderbird | ThunderCrab | Notes |
|---------|-------------|-------------|-------|
| IMAP4rev1 (RFC 3501) | ✅ | ⏳ stub (`Backend` trait only) | first concrete backend target |
| IMAP4rev2 (RFC 9051) | ✅ partial | ⏳ stub | post-r1 |
| SMTP submission (RFC 6409) | ✅ | ⏳ stub | use `lettre` |
| ManageSieve (RFC 5804) | ✅ | ⏳ stub | needed to push generated Sieve |
| Sieve (RFC 5228 + extensions) | ✅ via Pigeonhole | ✅ generation via `mail-config` | client editing arrives with GUI |
| JMAP (RFC 8620 / 8621) | ✅ since 102 | ❌ | second backend target |
| MIME (RFC 5322 / 2045–2049) | ✅ | ❌ | parser TBD — `mail-parser` crate |
| autoconfig (Mozilla) | ✅ | ❌ | server-side already done in `autoconfig.plausiden.com` |
| autodiscover (Outlook) | ✅ | ❌ | server-side already done |
| RFC 8689 (REQUIRETLS) | ✅ since 91 | ❌ | |
| MTA-STS (RFC 8461) | client-aware ✅ | ❌ | |

## Authentication

| Mechanism | Thunderbird | ThunderCrab | Notes |
|-----------|-------------|-------------|-------|
| LOGIN / PLAIN over TLS | ✅ | ⏳ stub | trivial |
| OAuth 2.0 (Gmail, Microsoft 365, Fastmail) | ✅ | ❌ | |
| GSSAPI / Kerberos | ✅ | ❌ | low priority |
| SCRAM-SHA-256 / SCRAM-SHA-1 | ✅ | ❌ | |

## Encryption

| Feature | Thunderbird | ThunderCrab | Notes |
|---------|-------------|-------------|-------|
| OpenPGP (RFC 4880, built-in since 78) | ✅ | ❌ | use `pgp` crate or shell to gnupg |
| S/MIME | ✅ | ❌ | |
| Hidden Reply-To via PGP/MIME headers | ✅ | ❌ | |

## UX / app-level

| Feature | Thunderbird | ThunderCrab | Notes |
|---------|-------------|-------------|-------|
| Threaded conversation view | ✅ | ❌ (no GUI yet) | needs References/In-Reply-To threading |
| Server-side search (IMAP SEARCH) | ✅ | ❌ | |
| Local full-text search index | ✅ (gloda) | ❌ | use `tantivy` once GUI exists |
| Address book (CardDAV) | ✅ | ❌ | |
| Calendar (CalDAV / iCalendar) | ✅ | ❌ | possibly out of scope |
| Filters / message rules editor UI | ✅ | ⏳ schema in `thundercrab-core` | UI lives in future GUI crate |
| **Federated rule learning** | ❌ | ✅ schema + safety + ledger primitives | ThunderCrab differentiator |
| **"Why is this here?" audit explainer** | ❌ | ⏳ design only | reads `X-PlausiDen-Category` and re-runs `evaluate` |

## Things ThunderCrab will deliberately NOT match

- Bundled Lightning calendar with full sync — out of scope.
- Built-in chat (XMPP/IRC/Matrix) — Thunderbird ships this; we won't.
- HTML compose with WYSIWYG editor — plain text + Markdown only on
  v1; HTML rendering for *received* mail will be sandboxed as soon as
  practical.
