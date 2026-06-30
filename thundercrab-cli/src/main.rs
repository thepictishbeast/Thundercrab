//! `crab` — ThunderCrab command-line front-end.
//!
//! Exercises the IMAP, ManageSieve, and SMTP modules end-to-end
//! against a real account. Useful as:
//!   * A smoke-test path for the wire layer (run `crab list` after
//!     dependency upgrades to confirm the protocol still works).
//!   * A bridge before the GUI lands — power users can manage rules
//!     and sample mail today, without waiting for toolkit selection.
//!   * A reference example for embedding ThunderCrab in scripts.
//!
//! Credentials come from environment variables — never from
//! flags — so passwords don't land in shell history:
//!
//! ```bash
//! export IMAP_HOST=mail.plausiden.com
//! export IMAP_USER=team@plausiden.com
//! export IMAP_PASSWORD=…
//! crab list
//! crab fetch INBOX --limit 10
//! crab push-sieve thundercrab path/to/script.sieve
//! crab send --to ops@example.com --subject Hi --body 'short msg'
//! ```

#![doc(html_no_source)]

use std::process::ExitCode;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use thundercrab_imap::{
    AccountConfig, Backend,
    managesieve,
    rust_imap::RustImapBackend,
    smtp::{OutboundMessage, SmtpEncryption, send_message},
};

#[derive(Parser, Debug)]
#[command(name = "crab", about = "ThunderCrab CLI — list/fetch/read/search/send/push-sieve")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// List every folder visible to the account, with message + unread counts.
    List,
    /// Print the most recent N message headers in a folder.
    Fetch {
        /// Folder name (e.g., `INBOX`, `Sent`, `Promotions`).
        folder: String,
        /// Limit (number of most-recent messages to fetch).
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
    /// Read one message's body (plain text; notes if a sanitized HTML part exists).
    Read {
        /// Folder name (e.g., `INBOX`).
        folder: String,
        /// IMAP UID of the message (see `fetch`).
        uid: u32,
    },
    /// Search a folder for a term (IMAP `SEARCH TEXT` — headers + body).
    Search {
        /// Folder name (e.g., `INBOX`).
        folder: String,
        /// Text to search for.
        term: String,
    },
    /// Save one attachment from a message to a local file. Get the index
    /// from `read` (the `[N]` in the attachments list).
    SaveAttachment {
        /// Folder name (e.g., `INBOX`).
        folder: String,
        /// IMAP UID of the message.
        uid: u32,
        /// Attachment index as shown by `read` (0-based).
        index: usize,
        /// Destination file path.
        out: std::path::PathBuf,
    },
    /// Push a Sieve script and set it active. Reads the script body
    /// from `path`. Empty file = clear-the-rules.
    PushSieve {
        /// Server-side script identifier (e.g., `thundercrab`).
        name: String,
        /// Path to the Sieve source on disk.
        path: std::path::PathBuf,
    },
    /// Send one plain-text message via SMTP submission.
    Send {
        /// Recipient (To: line). Repeat for multiple.
        #[arg(long)]
        to: Vec<String>,
        /// Cc recipient. Repeat for multiple.
        #[arg(long)]
        cc: Vec<String>,
        /// Subject line.
        #[arg(long)]
        subject: String,
        /// Plain-text body (always sent; the fallback part). Use
        /// `--body @file.txt` to read from a file.
        #[arg(long)]
        body: String,
        /// Optional HTML body. When set, the message goes out as
        /// `multipart/alternative` (plain + HTML). Use `--html @file.html`
        /// to read from a file.
        #[arg(long)]
        html: Option<String>,
        /// Request a read receipt (RFC 8098 `Disposition-Notification-To`),
        /// addressed to the sending account. Opt-in; recipients may ignore it.
        #[arg(long)]
        read_receipt: bool,
        /// Submission flavor — `starttls` (port 587) or `implicit` (port 465).
        #[arg(long, default_value = "starttls")]
        encryption: String,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,thundercrab_imap=info".into()),
        )
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("crab: {e:#}");
            ExitCode::from(1)
        }
    }
}

async fn run(cli: Cli) -> Result<()> {
    let cfg = load_config()?;
    let password = std::env::var("IMAP_PASSWORD")
        .context("IMAP_PASSWORD env var must be set")?;

    match cli.command {
        Cmd::List => cmd_list(&cfg, &password).await,
        Cmd::Fetch { folder, limit } => cmd_fetch(&cfg, &password, &folder, limit).await,
        Cmd::Read { folder, uid } => cmd_read(&cfg, &password, &folder, uid).await,
        Cmd::Search { folder, term } => cmd_search(&cfg, &password, &folder, &term).await,
        Cmd::SaveAttachment { folder, uid, index, out } => {
            cmd_save_attachment(&cfg, &password, &folder, uid, index, &out).await
        }
        Cmd::PushSieve { name, path } => cmd_push_sieve(&cfg, &password, &name, &path).await,
        Cmd::Send {
            to,
            cc,
            subject,
            body,
            html,
            read_receipt,
            encryption,
        } => {
            cmd_send(
                &cfg,
                &password,
                &to,
                &cc,
                &subject,
                &body,
                html.as_deref(),
                read_receipt,
                &encryption,
            )
            .await
        }
    }
}

fn load_config() -> Result<AccountConfig> {
    let host = std::env::var("IMAP_HOST").context("IMAP_HOST not set")?;
    let user = std::env::var("IMAP_USER").context("IMAP_USER not set")?;
    Ok(AccountConfig::plausiden(host, user))
}

async fn cmd_list(cfg: &AccountConfig, password: &str) -> Result<()> {
    let backend = RustImapBackend::connect(cfg, password)
        .await
        .map_err(|e| anyhow!("connect: {e}"))?;
    let folders = backend
        .list_folders()
        .await
        .map_err(|e| anyhow!("list_folders: {e}"))?;
    println!("{:>6}  {:>6}  {}", "UNREAD", "TOTAL", "FOLDER");
    for f in &folders {
        println!("{:>6}  {:>6}  {}", f.unseen, f.messages, f.name);
    }
    backend.logout().await;
    Ok(())
}

async fn cmd_fetch(
    cfg: &AccountConfig,
    password: &str,
    folder: &str,
    limit: u32,
) -> Result<()> {
    let backend = RustImapBackend::connect(cfg, password)
        .await
        .map_err(|e| anyhow!("connect: {e}"))?;
    let headers = backend
        .fetch_headers(folder, Some(limit))
        .await
        .map_err(|e| anyhow!("fetch_headers: {e}"))?;
    if headers.is_empty() {
        println!("(no messages in {folder})");
    } else {
        for h in &headers {
            println!("uid={}  from={}", h.uid, h.from);
            println!("    subject: {}", h.subject);
            for (k, v) in &h.other_headers {
                if matches!(k.as_str(), "list-id" | "list-unsubscribe" | "x-priority") {
                    println!("    {k}: {v}");
                }
            }
        }
    }
    backend.logout().await;
    Ok(())
}

async fn cmd_read(cfg: &AccountConfig, password: &str, folder: &str, uid: u32) -> Result<()> {
    let backend = RustImapBackend::connect(cfg, password)
        .await
        .map_err(|e| anyhow!("connect: {e}"))?;
    let body = backend
        .fetch_body(folder, uid)
        .await
        .map_err(|e| anyhow!("fetch_body: {e}"))?;
    match body.plain {
        Some(text) if !text.trim().is_empty() => println!("{text}"),
        _ => println!("(no plain-text body)"),
    }
    if let Some(html) = &body.html_sanitized {
        println!(
            "\n[+ {} bytes of sanitized HTML available — render it in the GUI/app]",
            html.len()
        );
    }
    if !body.attachments.is_empty() {
        println!("\nattachments ({}):", body.attachments.len());
        for (i, a) in body.attachments.iter().enumerate() {
            let name = if a.filename.is_empty() { "(unnamed)" } else { &a.filename };
            println!("  [{i}] {name}  {}  {} bytes", a.mime_type, a.size());
        }
        println!("  save with: crab save-attachment {folder} {uid} <index> <out-path>");
    }
    backend.logout().await;
    Ok(())
}

async fn cmd_search(cfg: &AccountConfig, password: &str, folder: &str, term: &str) -> Result<()> {
    let backend = RustImapBackend::connect(cfg, password)
        .await
        .map_err(|e| anyhow!("connect: {e}"))?;
    // Build a safe IMAP SEARCH criterion (strips control chars to block CRLF
    // injection, escapes quoted-specials). Single tested helper in the core.
    let query = thundercrab_imap::rust_imap::text_search_criterion(term);
    let hits = backend
        .search(folder, &query)
        .await
        .map_err(|e| anyhow!("search: {e}"))?;
    if hits.is_empty() {
        println!("(no matches for {term:?} in {folder})");
    } else {
        println!("{} match(es) for {term:?} in {folder}:", hits.len());
        for h in &hits {
            println!("uid={}  from={}", h.uid, h.from);
            println!("    subject: {}", h.subject);
        }
    }
    backend.logout().await;
    Ok(())
}

async fn cmd_save_attachment(
    cfg: &AccountConfig,
    password: &str,
    folder: &str,
    uid: u32,
    index: usize,
    out: &std::path::Path,
) -> Result<()> {
    let backend = RustImapBackend::connect(cfg, password)
        .await
        .map_err(|e| anyhow!("connect: {e}"))?;
    let body = backend
        .fetch_body(folder, uid)
        .await
        .map_err(|e| anyhow!("fetch_body: {e}"))?;
    let att = body.attachments.get(index).ok_or_else(|| {
        anyhow!(
            "no attachment at index {index}; message has {}",
            body.attachments.len()
        )
    })?;
    // `out` is a path the operator typed on their own command line (a clap
    // positional), exactly like `curl -o` or `cp` — writing there is the whole
    // point of the command. OS file permissions are the access control; we do
    // not restrict where the user may save their own mail.
    std::fs::write(out, &att.bytes)
        .with_context(|| format!("writing attachment to {}", out.display()))?;
    println!("ok: wrote {} bytes to {}", att.size(), out.display());
    backend.logout().await;
    Ok(())
}

async fn cmd_push_sieve(
    cfg: &AccountConfig,
    password: &str,
    name: &str,
    path: &std::path::Path,
) -> Result<()> {
    let script = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    managesieve::put_active_script(cfg, password, name, &script)
        .await
        .map_err(|e| anyhow!("put_active_script: {e}"))?;
    println!("ok: pushed {} bytes as `{}`, set active", script.len(), name);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cmd_send(
    cfg: &AccountConfig,
    password: &str,
    to: &[String],
    cc: &[String],
    subject: &str,
    body: &str,
    html: Option<&str>,
    read_receipt: bool,
    encryption: &str,
) -> Result<()> {
    if to.is_empty() {
        return Err(anyhow!("at least one --to recipient required"));
    }
    // `@file` reads the body/html from a local file the operator named on the
    // command line — intended CLI behavior, not untrusted input.
    let read_arg = |arg: &str, label: &str| -> Result<String> {
        if let Some(file) = arg.strip_prefix('@') {
            std::fs::read_to_string(file).with_context(|| format!("reading {label} from {file}"))
        } else {
            Ok(arg.to_string())
        }
    };
    let body_owned = read_arg(body, "body")?;
    let html_owned = html.map(|h| read_arg(h, "html")).transpose()?;
    let to_refs: Vec<&str> = to.iter().map(String::as_str).collect();
    let cc_refs: Vec<&str> = cc.iter().map(String::as_str).collect();
    let from = cfg.username.as_str();
    let msg = OutboundMessage {
        from,
        to: &to_refs,
        cc: &cc_refs,
        subject,
        body: &body_owned,
        html_body: html_owned.as_deref(),
        // Request the receipt to the sending account when --read-receipt is set.
        read_receipt_to: read_receipt.then_some(from),
    };
    let enc = match encryption {
        "starttls" => SmtpEncryption::StartTls,
        "implicit" => SmtpEncryption::ImplicitTls,
        other => return Err(anyhow!("unknown encryption {other:?}, expected starttls|implicit")),
    };
    send_message(cfg, password, enc, &msg)
        .await
        .map_err(|e| anyhow!("send_message: {e}"))?;
    println!("ok: sent");
    Ok(())
}
