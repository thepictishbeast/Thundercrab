//! End-to-end OFFLINE proof of the server-coupling pipeline, driving the real
//! exported FFI functions against a temp SQLite db:
//!
//!   synthetic flag events  ->  record_flag_event
//!                          ->  preview_suggestions (derive + safety-gate)
//!                          ->  rules_to_sieve       (emit personal Sieve)
//!                          ->  sievec               (Dovecot arbiter)
//!
//! No network, no live mailbox. This is exactly the chain that runs once a user
//! accepts a derived suggestion, minus the final ManageSieve push (which must
//! only ever target a real account with explicit operator consent).

use std::path::Path;
use std::process::Command;
use thundercrab_ffi::{
    preview_suggestions, record_flag_event, rules_to_sieve, FfiFlagEvent, FfiFlagSource,
};

fn ev(seed: u8, dest: &str) -> FfiFlagEvent {
    FfiFlagEvent {
        // Distinct 32-byte id per event; the store dedups via INSERT OR IGNORE.
        message_hash: vec![seed; 32],
        source: FfiFlagSource::ManualMove,
        destination: dest.to_string(),
        from_domain_with_at: "@github.com".to_string(),
        list_id: None,
        has_list_unsubscribe: false,
        subject_tokens: vec![],
        priority_high: false,
        observed_at: 1_700_000_000 + i64::from(seed),
    }
}

fn find_sievec() -> Option<String> {
    for p in ["/usr/bin/sievec", "/usr/lib/dovecot/sievec"] {
        if Path::new(p).exists() {
            return Some(p.to_string());
        }
    }
    None
}

#[test]
fn flag_events_become_gated_sievec_valid_script() {
    let db = std::env::temp_dir().join(format!("tc-e2e-{}.sqlite", std::process::id()));
    let dbs = db.to_string_lossy().to_string();
    let _ = std::fs::remove_file(&db);

    // A dominant pattern (9/10 of @github.com moved to "Dev") plus one stray to
    // INBOX. min_obs=3 drops the stray before the gate; the gate would also
    // refuse a file-into-INBOX rule (federated rules may only sort OUT of inbox).
    for i in 0..9u8 {
        record_flag_event(dbs.clone(), ev(i, "Dev")).expect("record Dev");
    }
    record_flag_event(dbs.clone(), ev(200, "INBOX")).expect("record stray");

    let suggestions = preview_suggestions(dbs.clone(), 3, 0.7).expect("preview");
    assert!(!suggestions.is_empty(), "expected one derived + gated suggestion");

    let sieve = rules_to_sieve(suggestions).expect("emit sieve");
    assert!(
        sieve.contains("address :domain :is \"from\" [\"github.com\"]"),
        "missing domain test:\n{sieve}"
    );
    assert!(sieve.contains("fileinto :create \"Dev\""), "missing fileinto:\n{sieve}");
    assert!(!sieve.contains("INBOX"), "gate must drop file-into-INBOX:\n{sieve}");

    if let Some(sievec) = find_sievec() {
        let dir = std::env::temp_dir().join(format!("tc-e2e-sv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let f = dir.join("personal.sieve");
        std::fs::write(&f, &sieve).expect("write");
        let out = Command::new(&sievec).arg(&f).output().expect("run sievec");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            out.status.success(),
            "sievec rejected the derived script:\n{}\n--- script ---\n{sieve}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let _ = std::fs::remove_file(&db);
}
