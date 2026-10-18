use axum::Json;
use axum::extract::{Query, State};
use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};

use super::AppState;
use crate::db;
use crate::domain::fx::SeriesStats;
use crate::error::{AppError, AppResult, bad};
use crate::fxsync;

#[derive(Serialize)]
pub struct Latest {
    date: NaiveDate,
    usd_pln: f64,
    usd_eur: f64,
    eur_pln: f64,
    usd_pln_change_1d: Option<f64>,
    usd_pln_change_30d: Option<f64>,
    stats_365d: Option<SeriesStats>,
}

pub async fn latest(State(st): State<AppState>) -> AppResult<Json<Latest>> {
    let series = db::load_series(&st.pool).await?;
    let (date, f) = series.latest().ok_or_else(|| AppError::Unavailable("no FX data yet; sync pending".into()))?;
    let prev = series.on(date - Days::new(1)).map(|p| f.usd_pln - p.usd_pln);
    let month = series.on(date - Days::new(30)).map(|p| f.usd_pln - p.usd_pln);
    Ok(Json(Latest {
        date,
        usd_pln: f.usd_pln,
        usd_eur: f.usd_eur,
        eur_pln: f.eur_pln(),
        usd_pln_change_1d: prev,
        usd_pln_change_30d: month,
        stats_365d: series.usd_pln_stats(date, 365),
    }))
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
}

#[derive(Serialize)]
pub struct HistoryPoint {
    date: NaiveDate,
    usd_pln: f64,
    usd_eur: f64,
    eur_pln: f64,
}

pub async fn history(State(st): State<AppState>, Query(q): Query<HistoryQuery>) -> AppResult<Json<Vec<HistoryPoint>>> {
    let today = st.clock.today();
    let to = q.to.unwrap_or(today);
    let from = q.from.unwrap_or(to - Days::new(365));
    if from > to {
        return Err(bad("from must be on or before to"));
    }
    let series = db::load_series(&st.pool).await?;
    Ok(Json(
        series
            .between(from, to)
            .map(|(d, f)| HistoryPoint { date: *d, usd_pln: f.usd_pln, usd_eur: f.usd_eur, eur_pln: f.eur_pln() })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct StatsQuery {
    lookback_days: Option<u64>,
}

pub async fn stats(State(st): State<AppState>, Query(q): Query<StatsQuery>) -> AppResult<Json<SeriesStats>> {
    let lookback = q.lookback_days.unwrap_or(365);
    if !(30..=3650).contains(&lookback) {
        return Err(bad("lookback_days must be in [30, 3650]"));
    }
    let series = db::load_series(&st.pool).await?;
    let (date, _) = series.latest().ok_or_else(|| AppError::Unavailable("no FX data yet".into()))?;
    series.usd_pln_stats(date, lookback).map(Json).ok_or_else(|| AppError::Unavailable("not enough FX history".into()))
}

#[derive(Serialize)]
pub struct SyncResult {
    written: usize,
    latest: Option<NaiveDate>,
}

pub async fn sync(State(st): State<AppState>) -> AppResult<Json<SyncResult>> {
    let written = fxsync::sync(&st.pool, &st.provider, st.config.fx_history_start, st.clock.today())
        .await
        .map_err(|e| AppError::Unavailable(format!("FX sync failed: {e:#}")))?;
    let latest = db::latest_fixing_date(&st.pool).await?;
    tracing::info!(written, ?latest, "fx sync on demand");
    Ok(Json(SyncResult { written, latest }))
}
