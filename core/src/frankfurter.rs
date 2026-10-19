//! ECB reference rates from a Frankfurter-compatible API: which ranges to fetch and how to read
//! the reply. The HTTP call itself belongs to the caller (reqwest on the server, fetch in the browser).

use std::collections::BTreeMap;

use chrono::{Days, NaiveDate};
use serde::Deserialize;

use crate::domain::fx::Fixing;

pub const SOURCE: &str = "ecb-frankfurter";
pub const DEFAULT_BASE_URL: &str = "https://api.frankfurter.dev/v1";
const CHUNK_DAYS: u64 = 366;
/// Re-fetch a few recent days in case a fixing was published late or revised.
const OVERLAP_DAYS: u64 = 7;

#[derive(Deserialize)]
struct TimeSeries {
    rates: BTreeMap<NaiveDate, BTreeMap<String, f64>>,
}

/// Date ranges still missing up to `today`, oldest first, each small enough for one request.
pub fn missing_ranges(latest: Option<NaiveDate>, history_start: NaiveDate, today: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut from = match latest {
        Some(latest) => latest.checked_sub_days(Days::new(OVERLAP_DAYS)).unwrap_or(latest),
        None => history_start,
    };
    let mut ranges = Vec::new();
    while from <= today {
        let to = (from + Days::new(CHUNK_DAYS - 1)).min(today);
        ranges.push((from, to));
        from = to + Days::new(1);
    }
    ranges
}

pub fn range_url(base_url: &str, from: NaiveDate, to: NaiveDate) -> String {
    format!("{}/{from}..{to}?base=USD&symbols=PLN,EUR", base_url.trim_end_matches('/'))
}

/// Parses a time-series reply; days missing either currency are skipped.
pub fn parse(body: &str) -> serde_json::Result<Vec<(NaiveDate, Fixing)>> {
    let ts: TimeSeries = serde_json::from_str(body)?;
    Ok(ts
        .rates
        .into_iter()
        .filter_map(|(date, r)| Some((date, Fixing { usd_pln: *r.get("PLN")?, usd_eur: *r.get("EUR")? })))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn empty_store_fetches_from_history_start_in_chunks() {
        let r = missing_ranges(None, d("2024-01-01"), d("2025-06-30"));
        assert_eq!(r, vec![(d("2024-01-01"), d("2024-12-31")), (d("2025-01-01"), d("2025-06-30"))]);
    }

    #[test]
    fn incremental_sync_overlaps_a_week() {
        assert_eq!(missing_ranges(Some(d("2026-10-16")), d("2016-01-01"), d("2026-10-19")), vec![(d("2026-10-09"), d("2026-10-19"))]);
    }

    #[test]
    fn parse_skips_incomplete_days() {
        let body = r#"{"rates":{"2026-10-15":{"PLN":3.9,"EUR":0.89},"2026-10-16":{"PLN":3.88}}}"#;
        assert_eq!(parse(body).unwrap(), vec![(d("2026-10-15"), Fixing { usd_pln: 3.9, usd_eur: 0.89 })]);
    }
}
