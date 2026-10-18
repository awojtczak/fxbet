//! Pulls ECB reference rates (USD base) from a Frankfurter-compatible API into the database.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{Days, NaiveDate};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::db;
use crate::domain::fx::Fixing;

const SOURCE: &str = "ecb-frankfurter";
const CHUNK_DAYS: u64 = 366;
/// Re-fetch a few recent days in case a fixing was published late or revised.
const OVERLAP_DAYS: u64 = 7;

pub struct FxProvider {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Deserialize)]
struct TimeSeries {
    rates: BTreeMap<NaiveDate, BTreeMap<String, f64>>,
}

impl FxProvider {
    pub fn new(base_url: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("fxbet/0.1 (self-hosted)")
            .build()?;
        Ok(Self { client, base_url: base_url.trim_end_matches('/').to_string() })
    }

    async fn fetch_range(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, Fixing)>> {
        let url = format!("{}/{from}..{to}?base=USD&symbols=PLN,EUR", self.base_url);
        let resp = self.client.get(&url).send().await.with_context(|| format!("GET {url}"))?;
        if !resp.status().is_success() {
            bail!("fx provider returned status={} url={url}", resp.status());
        }
        let body: TimeSeries = resp.json().await.context("decoding fx provider response")?;
        Ok(body
            .rates
            .into_iter()
            .filter_map(|(date, r)| Some((date, Fixing { usd_pln: *r.get("PLN")?, usd_eur: *r.get("EUR")? })))
            .collect())
    }
}

/// Fetch everything missing up to `today`. Returns the number of fixings written.
pub async fn sync(pool: &SqlitePool, provider: &FxProvider, history_start: NaiveDate, today: NaiveDate) -> Result<usize> {
    let mut from = match db::latest_fixing_date(pool).await? {
        Some(latest) => latest.checked_sub_days(Days::new(OVERLAP_DAYS)).unwrap_or(latest),
        None => history_start,
    };
    let mut written = 0;
    while from <= today {
        let to = (from + Days::new(CHUNK_DAYS - 1)).min(today);
        let fixings = provider.fetch_range(from, to).await?;
        written += db::upsert_fixings(pool, &fixings, SOURCE).await?;
        from = to + Days::new(1);
    }
    Ok(written)
}
