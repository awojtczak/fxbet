//! Pulls ECB reference rates (USD base) from a Frankfurter-compatible API into the database.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use fxbet_core::domain::fx::Fixing;
use fxbet_core::frankfurter;
use sqlx::SqlitePool;

use crate::db;

pub struct FxProvider {
    client: reqwest::Client,
    base_url: String,
}

impl FxProvider {
    pub fn new(base_url: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("fxbet/0.1 (self-hosted)")
            .build()?;
        Ok(Self { client, base_url })
    }

    async fn fetch_range(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, Fixing)>> {
        let url = frankfurter::range_url(&self.base_url, from, to);
        let resp = self.client.get(&url).send().await.with_context(|| format!("GET {url}"))?;
        if !resp.status().is_success() {
            bail!("fx provider returned status={} url={url}", resp.status());
        }
        let body = resp.text().await.with_context(|| format!("reading {url}"))?;
        frankfurter::parse(&body).context("decoding fx provider response")
    }
}

/// Fetch everything missing up to `today`. Returns the number of fixings written.
pub async fn sync(pool: &SqlitePool, provider: &FxProvider, history_start: NaiveDate, today: NaiveDate) -> Result<usize> {
    let latest = db::latest_fixing_date(pool).await?;
    let mut written = 0;
    for (from, to) in frankfurter::missing_ranges(latest, history_start, today) {
        let fixings = provider.fetch_range(from, to).await?;
        written += db::upsert_fixings(pool, &fixings, frankfurter::SOURCE).await?;
    }
    Ok(written)
}
