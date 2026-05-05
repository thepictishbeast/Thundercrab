//! `list_inbox` — manual smoke test for the IMAPS backend.
//!
//! Logs in to the configured server, lists every folder with its
//! message + unseen counts, fetches the most recent five header
//! summaries from INBOX, and logs out.
//!
//! Usage:
//!
//! ```bash
//! IMAP_HOST=mail.plausiden.com \
//! IMAP_USER=team@plausiden.com \
//! IMAP_PASSWORD=… \
//!     cargo run -p thundercrab-imap --example list_inbox
//! ```
//!
//! Not a unit test: it touches the network and a real mailbox. Run
//! it by hand when validating the backend against a new server, or
//! after upgrading async-imap / tokio-rustls.

use std::env;
use std::process::ExitCode;

use thundercrab_imap::{AccountConfig, Backend, rust_imap::RustImapBackend};

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

    let cfg = AccountConfig::plausiden(host, user);

    println!(
        "connecting to {}:{} as {}",
        cfg.imap_host, cfg.imap_port, cfg.username
    );
    let backend = match RustImapBackend::connect(&cfg, &pass).await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("connect failed: {e}");
            return ExitCode::from(1);
        }
    };

    println!("\n--- folders ---");
    match backend.list_folders().await {
        Ok(folders) => {
            for f in &folders {
                println!(
                    "  {:>6} unread / {:>6} total   {}",
                    f.unseen, f.messages, f.name
                );
            }
        }
        Err(e) => {
            eprintln!("list_folders failed: {e}");
            return ExitCode::from(1);
        }
    }

    println!("\n--- INBOX (most recent 5) ---");
    match backend.fetch_headers("INBOX", Some(5)).await {
        Ok(headers) => {
            for h in &headers {
                println!("  uid={} from={:?} subject={:?}", h.uid, h.from, h.subject);
            }
        }
        Err(e) => {
            eprintln!("fetch_headers failed: {e}");
            return ExitCode::from(1);
        }
    }

    backend.logout().await;
    ExitCode::SUCCESS
}
