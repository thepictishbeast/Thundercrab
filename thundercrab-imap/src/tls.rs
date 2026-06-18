//! Shared TLS client configuration and the post-quantum crypto posture.
//!
//! All of ThunderCrab's TLS — IMAPS and `ManageSieve` here, and SMTP via
//! lettre (which uses the same process-default provider) — runs on the
//! `aws-lc-rs` rustls provider. Its default key-exchange list is
//! **post-quantum hybrid first** (`X25519MLKEM768`), with classical groups
//! (`X25519`, `secp256r1`, `secp384r1`) as automatic fallback.
//!
//! SECURITY: hybrid key exchange defeats "harvest now, decrypt later" — a
//! network observer who records today's traffic cannot recover it with a
//! future quantum computer — *and* it never regresses connectivity: a server
//! that does not yet speak ML-KEM simply negotiates a classical group. The
//! [`CryptoMode`] knob lets the user force classical (compatibility) or require
//! PQ (hard guarantee, no fallback).
//!
//! BUG ASSUMPTION: lettre's `tokio1-rustls-tls` compiles in the `ring` provider
//! as well, so the `rustls` crate ends up with *two* providers and
//! `ClientConfig::builder()` has no unambiguous default. [`ensure_provider`]
//! installs `aws-lc-rs` as the process default once, which resolves the choice
//! for every rustls consumer in the process (including lettre's SMTP path).

use std::sync::{Arc, Once};

use rustls::ClientConfig;
use rustls::RootCertStore;
use rustls::crypto::CryptoProvider;
use rustls::crypto::aws_lc_rs;

/// How aggressively to use post-quantum key exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CryptoMode {
    /// Post-quantum hybrid (`X25519MLKEM768`) preferred, classical groups as
    /// automatic fallback. The recommended default: quantum-resistant where the
    /// server supports it, never a connectivity regression where it does not.
    #[default]
    PqHybrid,
    /// Classical key exchange only (`X25519`, `secp256r1`, `secp384r1`). For
    /// maximum compatibility or when PQ is explicitly not wanted.
    ClassicalOnly,
    /// Require a post-quantum hybrid group — the handshake FAILS rather than
    /// fall back to classical. For users who want a hard PQ guarantee.
    PqRequired,
}

/// Install the `aws-lc-rs` provider as the process default exactly once.
///
/// Idempotent: a second call (or a default already installed by something
/// else) is ignored. Call this before building any `rustls` config.
pub fn ensure_provider() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Err means a default was already installed; any installed aws-lc-rs
        // default is fine for us, so the result is intentionally ignored.
        let _ = aws_lc_rs::default_provider().install_default();
    });
}

/// Build the crypto provider for `mode`, adjusting only its key-exchange groups.
fn provider_for(mode: CryptoMode) -> Arc<CryptoProvider> {
    let mut provider = aws_lc_rs::default_provider();
    match mode {
        // The default list is already `X25519MLKEM768` first, then classical.
        CryptoMode::PqHybrid => {}
        CryptoMode::ClassicalOnly => {
            provider.kx_groups = vec![
                aws_lc_rs::kx_group::X25519,
                aws_lc_rs::kx_group::SECP256R1,
                aws_lc_rs::kx_group::SECP384R1,
            ];
        }
        CryptoMode::PqRequired => {
            provider.kx_groups = vec![
                aws_lc_rs::kx_group::X25519MLKEM768,
                aws_lc_rs::kx_group::SECP256R1MLKEM768,
            ];
        }
    }
    Arc::new(provider)
}

/// Build a rustls [`ClientConfig`] for `mode` with Mozilla's webpki roots and
/// no client auth — the shared config for IMAPS and `ManageSieve`.
///
/// BUG ASSUMPTION: `webpki_roots::TLS_SERVER_ROOTS` is well-formed, and the
/// `aws-lc-rs` provider always supports the default (TLS 1.2 + 1.3) versions —
/// TLS 1.3 is required for hybrid KEX — so `with_safe_default_protocol_versions`
/// cannot fail for these inputs.
#[must_use]
pub fn client_config(mode: CryptoMode) -> ClientConfig {
    ensure_provider();
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    ClientConfig::builder_with_provider(provider_for(mode))
        .with_safe_default_protocol_versions()
        .expect("aws-lc-rs provider supports default TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_is_pq_hybrid() {
        assert_eq!(CryptoMode::default(), CryptoMode::PqHybrid);
    }

    #[test]
    fn pq_hybrid_lists_post_quantum_group_first() {
        let p = provider_for(CryptoMode::PqHybrid);
        let first = p.kx_groups.first().expect("non-empty kx groups");
        assert_eq!(first.name(), rustls::NamedGroup::X25519MLKEM768);
    }

    #[test]
    fn classical_only_has_no_post_quantum_group() {
        let p = provider_for(CryptoMode::ClassicalOnly);
        assert!(
            !p.kx_groups
                .iter()
                .any(|g| g.name() == rustls::NamedGroup::X25519MLKEM768)
        );
    }

    #[test]
    fn pq_required_is_all_post_quantum() {
        let p = provider_for(CryptoMode::PqRequired);
        assert!(
            p.kx_groups
                .iter()
                .all(|g| matches!(
                    g.name(),
                    rustls::NamedGroup::X25519MLKEM768 | rustls::NamedGroup::secp256r1MLKEM768
                ))
        );
    }

    #[test]
    fn client_config_builds_for_every_mode() {
        for mode in [CryptoMode::PqHybrid, CryptoMode::ClassicalOnly, CryptoMode::PqRequired] {
            let _ = client_config(mode);
        }
    }
}
