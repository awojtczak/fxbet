use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::domain::fx::{RateSeries, SeriesStats};
use crate::error::{Error, Result, bad};

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

pub fn latest(series: &RateSeries) -> Result<Latest> {
    let (date, f) = series.latest().ok_or_else(|| Error::Unavailable("no FX data yet; sync pending".into()))?;
    let prev = series.on(date - Days::new(1)).map(|p| f.usd_pln - p.usd_pln);
    let month = series.on(date - Days::new(30)).map(|p| f.usd_pln - p.usd_pln);
    Ok(Latest {
        date,
        usd_pln: f.usd_pln,
        usd_eur: f.usd_eur,
        eur_pln: f.eur_pln(),
        usd_pln_change_1d: prev,
        usd_pln_change_30d: month,
        stats_365d: series.usd_pln_stats(date, 365),
    })
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[derive(Serialize)]
pub struct HistoryPoint {
    date: NaiveDate,
    usd_pln: f64,
    usd_eur: f64,
    eur_pln: f64,
}

pub fn history(series: &RateSeries, q: &HistoryQuery, today: NaiveDate) -> Result<Vec<HistoryPoint>> {
    let to = q.to.unwrap_or(today);
    let from = q.from.unwrap_or(to - Days::new(365));
    if from > to {
        return Err(bad("from must be on or before to"));
    }
    Ok(series
        .between(from, to)
        .map(|(d, f)| HistoryPoint { date: *d, usd_pln: f.usd_pln, usd_eur: f.usd_eur, eur_pln: f.eur_pln() })
        .collect())
}

#[derive(Deserialize)]
pub struct StatsQuery {
    pub lookback_days: Option<u64>,
}

pub fn stats(series: &RateSeries, q: &StatsQuery) -> Result<SeriesStats> {
    let lookback = q.lookback_days.unwrap_or(365);
    if !(30..=3650).contains(&lookback) {
        return Err(bad("lookback_days must be in [30, 3650]"));
    }
    let (date, _) = series.latest().ok_or_else(|| Error::Unavailable("no FX data yet".into()))?;
    series.usd_pln_stats(date, lookback).ok_or_else(|| Error::Unavailable("not enough FX history".into()))
}

#[derive(Serialize)]
pub struct SyncResult {
    pub written: usize,
    pub latest: Option<NaiveDate>,
}
