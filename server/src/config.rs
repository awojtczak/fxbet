use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    pub static_dir: PathBuf,
    /// Frankfurter-compatible API (ECB reference rates), no key required.
    pub fx_provider_url: String,
    pub fx_sync_interval_hours: u64,
    /// First date fetched on an empty database.
    pub fx_history_start: NaiveDate,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            bind: var_or("FXBET_BIND", "0.0.0.0:8080").parse().context("FXBET_BIND")?,
            database_url: var_or("FXBET_DATABASE_URL", "sqlite://data/fxbet.db?mode=rwc"),
            static_dir: var_or("FXBET_STATIC_DIR", "../web/dist").into(),
            fx_provider_url: var_or("FXBET_FX_PROVIDER_URL", fxbet_core::frankfurter::DEFAULT_BASE_URL),
            fx_sync_interval_hours: var_or("FXBET_FX_SYNC_HOURS", "6").parse().context("FXBET_FX_SYNC_HOURS")?,
            fx_history_start: var_or("FXBET_FX_HISTORY_START", "2016-01-01").parse().context("FXBET_FX_HISTORY_START")?,
        })
    }
}

fn var_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}
