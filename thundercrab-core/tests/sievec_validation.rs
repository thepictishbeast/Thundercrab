//! The emitted Sieve must compile under Dovecot's `sievec` — the real arbiter
//! (RFC 5228 syntax + Dovecot acceptance), mirroring how the server validates
//! `categories.sieve`. Skips gracefully when `sievec` is absent (non-prime CI),
//! so it enforces only where the tool exists.

use std::path::Path;
use std::process::Command;
use thundercrab_core::{to_sieve, Action, CrabRule, MatchExpr, RuleOrigin};

fn find_sievec() -> Option<String> {
    for p in ["/usr/bin/sievec", "/usr/lib/dovecot/sievec", "/usr/local/bin/sievec"] {
        if Path::new(p).exists() {
            return Some(p.to_string());
        }
    }
    None
}

fn r(id: &str, score: i32, stop: bool, when: MatchExpr, action: Action) -> CrabRule {
    CrabRule {
        id: id.to_string(),
        display_name: format!("{id} display"),
        when,
        action,
        score,
        stop_on_match: stop,
        origin: RuleOrigin::Federated,
    }
}

/// A ruleset exercising every MatchExpr + Action shape the emitter handles.
fn representative_rules() -> Vec<CrabRule> {
    vec![
        r(
            "from_github",
            90,
            true,
            MatchExpr::FromDomainIn { domains: vec!["@github.com".into(), "@notifications.github.com".into()] },
            Action::FileInto { folder: "Dev/GitHub".into() },
        ),
        r(
            "promo",
            60,
            true,
            MatchExpr::SubjectContainsAny { needles: vec!["sale".into(), "50% off".into(), "unsubscribe".into()] },
            Action::FileInto { folder: "Promotions".into() },
        ),
        r(
            "mailing_list",
            55,
            false,
            MatchExpr::All {
                exprs: vec![
                    MatchExpr::HasHeader { header: "List-Id".into() },
                    MatchExpr::Not {
                        expr: Box::new(MatchExpr::HeaderContains {
                            header: "X-Priority".into(),
                            substring: "1".into(),
                        }),
                    },
                ],
            },
            Action::Sequence {
                actions: vec![
                    Action::SetFlag { flag: "\\Seen".into() },
                    Action::FileInto { folder: "Lists".into() },
                ],
            },
        ),
        r(
            "vip",
            70,
            false,
            MatchExpr::Any {
                exprs: vec![
                    MatchExpr::HeaderContains { header: "From".into(), substring: "ceo@".into() },
                    MatchExpr::HeaderContains { header: "Subject".into(), substring: "[URGENT]".into() },
                ],
            },
            Action::SetFlag { flag: "\\Flagged".into() },
        ),
        r(
            "catch_all",
            1,
            false,
            MatchExpr::Always,
            Action::FileInto { folder: "Catch \"All\"".into() }, // exercises quote escaping
        ),
    ]
}

#[test]
fn emitted_sieve_compiles_under_sievec() {
    let script = to_sieve(&representative_rules());

    let Some(sievec) = find_sievec() else {
        eprintln!("SKIP: sievec not found (install Dovecot to enforce). Script was:\n{script}");
        return;
    };

    let dir = std::env::temp_dir().join(format!("tc-sievec-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir tmp");
    let path = dir.join("personal.sieve");
    std::fs::write(&path, &script).expect("write script");

    let out = Command::new(&sievec).arg(&path).output().expect("run sievec");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        out.status.success(),
        "sievec rejected the emitted script.\n--- stderr ---\n{}\n--- stdout ---\n{}\n--- script ---\n{script}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout),
    );
}
