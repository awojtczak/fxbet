mod accounts;
mod analyze;
mod bets;
mod fx;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post};
use sqlx::SqlitePool;

use crate::clock::Clock;
use crate::config::Config;
use crate::error::{AppResult, bad};
use crate::fxsync::FxProvider;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub clock: Arc<dyn Clock>,
    pub provider: Arc<FxProvider>,
    pub config: Arc<Config>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/fx/latest", get(fx::latest))
        .route("/api/fx/history", get(fx::history))
        .route("/api/fx/stats", get(fx::stats))
        .route("/api/fx/sync", post(fx::sync))
        .route("/api/accounts", get(accounts::list).post(accounts::create))
        .route("/api/accounts/{id}", axum::routing::put(accounts::update).delete(accounts::remove))
        .route("/api/accounts/{id}/rates", post(accounts::add_rate))
        .route("/api/accounts/{id}/rates/{rate_id}", delete(accounts::remove_rate))
        .route("/api/analyze", post(analyze::analyze))
        .route("/api/bets", get(bets::list).post(bets::create))
        .route("/api/bets/{id}", get(bets::detail).patch(bets::update).delete(bets::remove))
        .route("/api/bets/{id}/conversions", post(bets::add_conversion))
        .route("/api/bets/{id}/conversions/{conversion_id}", delete(bets::remove_conversion))
        .route("/api/calibration", get(bets::calibration))
        .with_state(state)
}

// ---------- boundary validation helpers ----------

fn positive(name: &str, v: f64) -> AppResult<f64> {
    if v.is_finite() && v > 0.0 { Ok(v) } else { Err(bad(format!("{name} must be a positive number"))) }
}

fn pct(name: &str, v: f64) -> AppResult<f64> {
    if v.is_finite() && (0.0..100.0).contains(&v) { Ok(v) } else { Err(bad(format!("{name} must be in [0, 100)"))) }
}

fn prob(name: &str, v: f64) -> AppResult<f64> {
    if v.is_finite() && (0.0..=1.0).contains(&v) { Ok(v) } else { Err(bad(format!("{name} must be in [0, 1]"))) }
}

fn non_empty(name: &str, v: &str) -> AppResult<String> {
    let t = v.trim();
    if t.is_empty() || t.len() > 200 { Err(bad(format!("{name} must be 1-200 characters"))) } else { Ok(t.to_string()) }
}
