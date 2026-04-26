# Federated Suggestion Heuristics

The current `derive_rule_candidates` is a single algorithm:
"if a domain consistently routes to one folder ≥ N times with ≥ X%
dominance, propose `FromDomainIn([domain]) → FileInto(folder)`".

That's deliberately the floor. Better heuristics live behind future
flags. This doc lists what's considered, what's not, and why.

## Currently shipped

### Domain-dominance (`derive::derive_rule_candidates`)
- **Inputs:** `(from_domain, destination, count)` rows from
  `Db::flag_counts_by_domain_dest`.
- **Defaults:** `min_observations = 3`, `dominance_threshold = 0.7`.
- **Output:** at most one rule per `(domain, dest)` pair.
- **Strengths:** trivial to derive; easy to corroborate (multiple
  installs naturally converge on the same domain→folder for popular
  senders).
- **Weaknesses:** misses List-Id-based forums, multi-feature signals,
  per-list mailing-list patterns where the domain is generic
  (e.g., `@googlegroups.com` covers thousands of distinct lists).

## Future heuristics (not yet implemented)

### List-Id-driven
For mailing lists (`List-Id` header present), derive
`HasHeader{List-Id} ∧ HeaderContains{List-Id, "<list-name>"}` →
`FileInto(<folder>)`. Beats domain-dominance for googlegroups.com,
freelists.org, etc.

### List-Unsubscribe + sender combo
A rule like `HasHeader{List-Unsubscribe} ∧ FromDomainIn{[@x.com]}`
catches "this newsletter from x.com" without conflating x.com's
non-newsletter mail.

### Subject-token tendency
"This sender + these subject tokens consistently route here" —
useful for `@stripe.com` where receipts vs. account alerts go to
different folders. Risk: subject tokens can carry PII; only ship
tokens that survive a deny-list check (no email addresses, no
URLs, no numbers > 4 digits).

### Negative rules
"User repeatedly *moves out* of folder X back to INBOX" suggests the
existing rule is wrong. The current schema has no `Not` action; a
future variant could be "demote score on disagreement signal."

## Heuristics deliberately NOT considered

- **Body content matching.** The match AST has no body variant. This
  is a hard line; the privacy guarantee depends on it.
- **Recipient-based rules.** The user's own address(es) are not on
  the wire format. A rule like "messages To: bigcustomer@…" would
  leak recipient-side relationships if federated; we'd only allow
  it as a local-only rule, never submitted.
- **Time-of-day patterns.** Anti-pattern for traffic analysis.
- **Per-message-frequency rules** ("you get one a week from this
  sender"). Frequency isn't useful enough to justify the metadata
  shipping.

## Corroboration math (when implemented on the ledger side)

The ledger never sees this code, but for context:

- One install signs and submits → 1 corroboration vote.
- N independent installs propose the *same* `pattern_hash` →
  candidate enters the public suggestion list.
- A pulled suggestion shows up in client UI as
  "X others also use this rule" — never names, never counts beyond
  log-bucketed orders of magnitude.

## How to add a new heuristic

1. Add a function to `thundercrab-suggestions/src/derive.rs` that
   takes a different input shape from `Db` (probably a new
   `Db::flag_counts_by_<feature>` query).
2. Verify the output rules pass `safety::is_safe_match` and
   `safety::apply_suggestion`.
3. Add a row to this doc.
4. Bump the schema version in `Db::SCHEMA_VERSION` if the new query
   needs a new index.
