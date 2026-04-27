//! `crab-ledger` binary — runs the federated suggestion ledger.
//!
//! Default bind: `127.0.0.1:8090`. Production deploys put this behind
//! a Tor hidden service descriptor or, optionally, behind nginx for
//! clearnet access. Either way the public surface is the three `/v1/*`
//! endpoints documented in `api.rs`.
//!
//! Environment variables:
//!   * `CRAB_LEDGER_BIND`     — listen address (default `127.0.0.1:8090`)
//!   * `CRAB_LEDGER_DB`       — SQLite path (default `./crab-ledger.db`)
//!
//! No config file. Re-read of env on restart is the configuration model.

#![doc(html_no_source)]

use crab_ledger::{Store, api::AppState, router};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::EnvFilter;

const DEFAULT_BIND: &str = "127.0.0.1:8090";
const DEFAULT_DB: &str = "./crab-ledger.db";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_target(false)
        .compact()
        .init();

    let bind: SocketAddr = std::env::var("CRAB_LEDGER_BIND")
        .unwrap_or_else(|_| DEFAULT_BIND.into())
        .parse()?;
    let db = std::env::var("CRAB_LEDGER_DB").unwrap_or_else(|_| DEFAULT_DB.into());

    let store = Store::open(&db)?;
    let state = Arc::new(AppState::new(store));

    let app = router(state);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    info!(%bind, %db, "crab-ledger listening");

    axum::serve(listener, app).await?;
    Ok(())
}
