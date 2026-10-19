use axum::Json;
use axum::extract::{Query, State};
use fxbet_core::domain::fx::SeriesStats;
use fxbet_core::service::fx::{self, HistoryPoint, HistoryQuery, Latest, StatsQuery, SyncResult};

use super::AppState;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::fxsync;

pub async fn latest(State(st): State<AppState>) -> AppResult<Json<Latest>> {
    Ok(Json(fx::latest(&db::load_series(&st.pool).await?)?))
}

pub async fn history(State(st): State<AppState>, Query(q): Query<HistoryQuery>) -> AppResult<Json<Vec<HistoryPoint>>> {
    Ok(Json(fx::history(&db::load_series(&st.pool).await?, &q, st.clock.today())?))
}

pub async fn stats(State(st): State<AppState>, Query(q): Query<StatsQuery>) -> AppResult<Json<SeriesStats>> {
    Ok(Json(fx::stats(&db::load_series(&st.pool).await?, &q)?))
}

pub async fn sync(State(st): State<AppState>) -> AppResult<Json<SyncResult>> {
    let written = fxsync::sync(&st.pool, &st.provider, st.config.fx_history_start, st.clock.today())
        .await
        .map_err(|e| AppError::Unavailable(format!("FX sync failed: {e:#}")))?;
    let latest = db::latest_fixing_date(&st.pool).await?;
    tracing::info!(written, ?latest, "fx sync on demand");
    Ok(Json(SyncResult { written, latest }))
}
