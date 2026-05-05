//! Federated suggestion ledger — submission/verification primitives.
//!
//! Wire format for a submission:
//!
//! ```text
//! SignedSuggestion {
//!   suggestion: Suggestion {                     // canonical-encoded
//!     rule:           CrabRule,                  // safe-by-construction
//!     pattern_hash:   blake3-32(rule.canonical), // dedup key
//!     submitted_at_day: i64,                     // UTC day index, NOT exact ts
//!   },
//!   public_key: [u8; 32],                        // Ed25519 pubkey
//!   signature:  [u8; 64],                        // Ed25519 over canonical Suggestion
//! }
//! ```
//!
//! The ledger validates the signature, dedupes by
//! `(pattern_hash, public_key)`, and counts distinct corroborators per
//! `pattern_hash`.
//!
//! SECURITY:
//!   * `submitted_at_day` is days-since-epoch (UTC), so submissions
//!     don't carry minute-precision timing useful for traffic analysis.
//!   * `public_key` is per-install (rotate by deleting the keypair
//!     file), not per-user — one user with two devices submits twice
//!     and corroborates themselves. Mitigation lives on the ledger
//!     (require N≥3 with reputation) not here.
//!   * Canonical encoding uses `serde_json::to_vec` with sorted keys
//!     via the `canonical_json` helper to avoid signature mismatches
//!     from key reordering.

use chrono::Utc;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use thundercrab_core::CrabRule;

/// A suggestion ready for signing — the body the signature covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    /// The proposed rule. Must be safety-checked by the caller (see
    /// [`super::safety::is_safe_match`] and the action checks in
    /// [`super::safety::apply_suggestion`]).
    pub rule: CrabRule,
    /// blake3-32 of the canonical-JSON-encoded `rule` — used as the
    /// ledger's dedup / corroboration key. Two submissions with the
    /// same `pattern_hash` count as agreeing.
    pub pattern_hash: [u8; 32],
    /// UTC days since the Unix epoch — coarsened from full timestamp
    /// for traffic-analysis resistance.
    pub submitted_at_day: i64,
}

impl Suggestion {
    /// Build from a rule, computing `pattern_hash` and using today's
    /// UTC day index.
    #[must_use]
    pub fn new(rule: CrabRule) -> Self {
        let canonical = canonical_json_bytes(&rule);
        let pattern_hash: [u8; 32] = *blake3::hash(&canonical).as_bytes();
        Self {
            rule,
            pattern_hash,
            submitted_at_day: today_day_index(),
        }
    }
}

/// A signed suggestion as it travels over the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedSuggestion {
    /// The signed body.
    pub suggestion: Suggestion,
    /// Ed25519 public key of the submitting install.
    #[serde(with = "hex_array_32")]
    pub public_key: [u8; 32],
    /// Ed25519 signature over the canonical encoding of `suggestion`.
    #[serde(with = "hex_array_64")]
    pub signature: [u8; 64],
}

/// Per-install signing key. Persist the secret bytes mode-0600.
pub struct InstallKey {
    signing: SigningKey,
}

impl InstallKey {
    /// Generate a fresh keypair using the OS RNG.
    #[must_use]
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::generate(&mut OsRng),
        }
    }

    /// Reconstitute from a 32-byte secret key.
    #[must_use]
    pub fn from_secret(secret: [u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(&secret),
        }
    }

    /// Raw 32 bytes of the secret. Persist with care.
    #[must_use]
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }

    /// 32-byte public key.
    #[must_use]
    pub fn public_bytes(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// Sign a `Suggestion`, producing a `SignedSuggestion` ready to
    /// submit to the ledger.
    pub fn sign(&self, suggestion: Suggestion) -> SignedSuggestion {
        let body = canonical_json_bytes(&suggestion);
        let sig: Signature = self.signing.sign(&body);
        SignedSuggestion {
            suggestion,
            public_key: self.public_bytes(),
            signature: sig.to_bytes(),
        }
    }
}

/// Verification errors.
#[derive(Debug, Error)]
pub enum LedgerError {
    /// Signature does not match the public key + body.
    #[error("invalid signature")]
    BadSignature,
    /// Public key is not a valid Ed25519 point.
    #[error("invalid public key")]
    BadKey,
    /// `pattern_hash` does not match the rule's actual hash —
    /// submitter is lying about which rule they're corroborating.
    #[error("pattern hash mismatch")]
    HashMismatch,
}

/// Verify a `SignedSuggestion` end-to-end:
///   1. signature is valid for the declared public key + canonical body,
///   2. `pattern_hash` actually matches the rule's canonical hash.
///
/// Returns the verified suggestion on success.
///
/// # Errors
/// `LedgerError::BadKey` when the public-key bytes don't decode;
/// `LedgerError::BadSignature` when the signature fails to verify;
/// `LedgerError::HashMismatch` when the rule's canonical hash
/// doesn't match the declared `pattern_hash`.
pub fn verify(signed: &SignedSuggestion) -> Result<&Suggestion, LedgerError> {
    let pk = VerifyingKey::from_bytes(&signed.public_key).map_err(|_| LedgerError::BadKey)?;
    let body = canonical_json_bytes(&signed.suggestion);
    let sig = Signature::from_bytes(&signed.signature);
    pk.verify(&body, &sig)
        .map_err(|_| LedgerError::BadSignature)?;
    let recomputed: [u8; 32] =
        *blake3::hash(&canonical_json_bytes(&signed.suggestion.rule)).as_bytes();
    if recomputed != signed.suggestion.pattern_hash {
        return Err(LedgerError::HashMismatch);
    }
    Ok(&signed.suggestion)
}

/// Canonical JSON encoding — sorted keys, compact, UTF-8. Used as the
/// signing input AND as the input to `pattern_hash`. Must be byte-stable
/// across Thundercrab versions; keep this function free of incidental
/// changes.
fn canonical_json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    // serde_json with `BTreeMap` semantics on the way out; we accomplish
    // this by going through `Value` and re-serializing. Slightly slower
    // than streaming, but the inputs are tiny rules.
    let v: serde_json::Value = serde_json::to_value(value).expect("infallible serialize");
    let canonical = sort_value(v);
    serde_json::to_vec(&canonical).expect("infallible reserialize")
}

fn sort_value(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = serde_json::Map::new();
            for (k, child) in entries {
                out.insert(k, sort_value(child));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(sort_value).collect())
        }
        other => other,
    }
}

/// UTC day index since Unix epoch.
fn today_day_index() -> i64 {
    Utc::now().timestamp() / 86_400
}

mod hex_array_32 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(b: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(b))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let s = String::deserialize(d)?;
        let v = hex::decode(&s).map_err(serde::de::Error::custom)?;
        let arr: [u8; 32] = v
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 32 bytes"))?;
        Ok(arr)
    }
}

mod hex_array_64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(b: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(b))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let s = String::deserialize(d)?;
        let v = hex::decode(&s).map_err(serde::de::Error::custom)?;
        let arr: [u8; 64] = v
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 64 bytes"))?;
        Ok(arr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thundercrab_core::{Action, MatchExpr, RuleOrigin};

    fn sample_rule() -> CrabRule {
        CrabRule {
            id: "promotions_listunsub".into(),
            display_name: "Promotions".into(),
            when: MatchExpr::HasHeader {
                header: "List-Unsubscribe".into(),
            },
            action: Action::FileInto {
                folder: "Promotions".into(),
            },
            score: 30,
            stop_on_match: true,
            origin: RuleOrigin::User,
        }
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let key = InstallKey::generate();
        let signed = key.sign(Suggestion::new(sample_rule()));
        verify(&signed).unwrap();
    }

    #[test]
    fn tampered_rule_fails_verification() {
        let key = InstallKey::generate();
        let mut signed = key.sign(Suggestion::new(sample_rule()));
        signed.suggestion.rule.action = Action::FileInto {
            folder: "INBOX".into(),
        };
        // Changing the rule changes the canonical body; signature can't
        // match anymore, AND pattern_hash now mismatches.
        let err = verify(&signed).unwrap_err();
        assert!(matches!(
            err,
            LedgerError::BadSignature | LedgerError::HashMismatch
        ));
    }

    #[test]
    fn tampered_pattern_hash_caught() {
        let key = InstallKey::generate();
        let mut signed = key.sign(Suggestion::new(sample_rule()));
        // Forge a hash that matches what we WANT the signature to claim
        // (a different rule), but keep the actual rule the original.
        signed.suggestion.pattern_hash = [0xff; 32];
        // The signature now also covers the bogus pattern_hash because
        // we changed it post-sign — so we'd expect BadSignature first.
        // But we're testing the path where signature matches body but
        // pattern_hash != hash(rule). Re-sign over the tampered body:
        let tampered_body = canonical_json_bytes(&signed.suggestion);
        let sig = key.signing.sign(&tampered_body);
        signed.signature = sig.to_bytes();
        let err = verify(&signed).unwrap_err();
        assert!(matches!(err, LedgerError::HashMismatch));
    }

    #[test]
    fn pattern_hash_is_deterministic_across_calls() {
        let a = Suggestion::new(sample_rule());
        let b = Suggestion::new(sample_rule());
        assert_eq!(a.pattern_hash, b.pattern_hash);
    }

    #[test]
    fn signed_suggestion_round_trips_json() {
        let key = InstallKey::generate();
        let signed = key.sign(Suggestion::new(sample_rule()));
        let j = serde_json::to_string(&signed).unwrap();
        let back: SignedSuggestion = serde_json::from_str(&j).unwrap();
        assert_eq!(signed, back);
        verify(&back).unwrap();
    }

    #[test]
    fn install_key_persistence_round_trip() {
        let k1 = InstallKey::generate();
        let secret = k1.secret_bytes();
        let k2 = InstallKey::from_secret(secret);
        assert_eq!(k1.public_bytes(), k2.public_bytes());
        let s = k1.sign(Suggestion::new(sample_rule()));
        let s2 = k2.sign(s.suggestion.clone());
        // Same key signs same body the same way.
        assert_eq!(s.signature, s2.signature);
    }
}
