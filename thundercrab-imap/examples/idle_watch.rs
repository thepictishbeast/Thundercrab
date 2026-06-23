//! `idle_watch` — manual smoke test for the IMAP IDLE push watcher.
//!
//! Logs in, parks on INBOX in IDLE, and prints a line each time the
//! server reports activity (send yourself a message to see it fire) or
//! the re-arm timeout elapses. Ctrl-C to stop.
//!
//! Usage:
//!
//! ```bash
//! IMAP_HOST=mail.plausiden.com \
//! IMAP_USER=team@plausiden.com \
//! IMAP_PASSWORD=… \
//! IDLE_SECS=120 \
//!     cargo run -p thundercrab-imap --example idle_watch
//! ```
//!
//! `IDLE_SECS` (default 120) shortens the re-arm window so the loop is
//! observable by hand; production uses [`thundercrab_imap::idle::IDLE_REARM`]
//! (29 min) via `wait_rearm`.
//!
//! Not a unit test: it touches the network and a real mailbox.

use std::env;
use std::process::ExitCode;
use std::time::Duration;

use thundercrab_imap::AccountConfig;
use thundercrab_imap::idle::{IdleEvent, IdleWatcher};

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let host = env::var("IMAP_HOST").unwrap_or_else(|_| "mail.plausiden.com".into());
    let Ok(user) = env::var("IMAP_USER") else {
        eprintln!("error: IMAP_USER not set");
        return ExitCode::from(2);
    };
    let Ok(pass) = env::var("IMAP_PASSWORD") else {
        eprintln!("error: IMAP_PASSWORD not set");
        return ExitCode::from(2);
    };
    let secs: u64 = env::var("IDLE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);

    let cfg = AccountConfig::plausiden(host, user);
    println!(
        "connecting to {}:{} as {} — IDLE on INBOX (re-arm every {secs}s)",
        cfg.imap_host, cfg.imap_port, cfg.username
    );

    let mut watcher = match IdleWatcher::connect(&cfg, &pass, "INBOX").await {
        Ok(w) => w,
        Err(e) => {
            eprintln!("connect failed: {e}");
            return ExitCode::from(1);
        }
    };

    let rearm = Duration::from_secs(secs);
    loop {
        match watcher.wait(rearm).await {
            Ok((IdleEvent::Activity, next)) => {
                println!("● activity on INBOX — (re-fetch headers here)");
                watcher = next;
            }
            Ok((IdleEvent::Idle, next)) => {
                println!("· quiet; re-arming IDLE");
                watcher = next;
            }
            Err(e) => {
                eprintln!("idle failed (connection likely dropped): {e}");
                return ExitCode::from(1);
            }
        }
    }
}
