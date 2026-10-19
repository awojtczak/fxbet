mod api;
mod clock;
mod config;
mod db;
mod error;
mod fxsync;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::http::{HeaderValue, header};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::api::AppState;
use crate::clock::{Clock, SystemClock};
use crate::config::Config;
use crate::fxsync::FxProvider;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=info,sqlx=warn".into()))
        .init();

    let config = Arc::new(Config::from_env()?);
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let pool = db::connect(&config.database_url).await?;
    db::seed_default_accounts(&pool, clock.today()).await?;
    let provider = Arc::new(FxProvider::new(config.fx_provider_url.clone())?);
    let state = AppState { pool, clock, provider, config: config.clone() };

    spawn_fx_sync(state.clone());

    let index = config.static_dir.join("index.html");
    let spa = ServeDir::new(&config.static_dir).fallback(ServeFile::new(index));
    let app = api::router(state)
        .fallback_service(spa)
        .layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-cache")))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(bind = %config.bind, static_dir = %config.static_dir.display(), "fxbet listening");
    axum::serve(listener, app).with_graceful_shutdown(shutdown()).await?;
    Ok(())
}

/// Syncs immediately, then on an interval. Failures are logged and retried next tick;
/// the app keeps serving from whatever history it already has.
fn spawn_fx_sync(st: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(st.config.fx_sync_interval_hours.max(1) * 3600));
        loop {
            tick.tick().await;
            match fxsync::sync(&st.pool, &st.provider, st.config.fx_history_start, st.clock.today()).await {
                Ok(written) => tracing::info!(written, "fx sync done"),
                Err(e) => tracing::warn!(error = %format!("{e:#}"), "fx sync failed"),
            }
        }
    });
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = term => {}
    }
    tracing::info!("shutting down");
}
