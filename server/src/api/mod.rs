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
use crate::error::AppError;
use crate::fxsync::FxProvider;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub clock: Arc<dyn Clock>,
    pub provider: Arc<FxProvider>,
    pub config: Arc<Config>,
}

/// Runs CPU-bound model code (Monte Carlo) off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> fxbet_core::Result<T> + Send + 'static) -> Result<T, AppError> {
    Ok(tokio::task::spawn_blocking(f).await.map_err(|e| AppError::Internal(e.into()))??)
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
