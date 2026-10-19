//! Daily FX fixings (USD base) and statistics over them.

use std::collections::BTreeMap;

use chrono::{Days, NaiveDate};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    Usd,
    Eur,
    Pln,
}

impl Currency {
    pub fn as_str(self) -> &'static str {
        match self {
            Currency::Usd => "USD",
            Currency::Eur => "EUR",
            Currency::Pln => "PLN",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "USD" => Some(Currency::Usd),
            "EUR" => Some(Currency::Eur),
            "PLN" => Some(Currency::Pln),
            _ => None,
        }
    }
}

/// One fixing: how many PLN and EUR one USD buys.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Fixing {
    pub usd_pln: f64,
    pub usd_eur: f64,
}

impl Fixing {
    pub fn eur_pln(&self) -> f64 {
        self.usd_pln / self.usd_eur
    }
}

/// Business-day fixings. Lookups on weekends/holidays forward-fill the last fixing.
#[derive(Debug, Clone, Default)]
pub struct RateSeries {
    fixings: BTreeMap<NaiveDate, Fixing>,
}

impl RateSeries {
    pub fn new(fixings: BTreeMap<NaiveDate, Fixing>) -> Self {
        Self { fixings }
    }

    pub fn latest(&self) -> Option<(NaiveDate, Fixing)> {
        self.fixings.iter().next_back().map(|(d, f)| (*d, *f))
    }

    /// Last fixing on or before `date`.
    pub fn on(&self, date: NaiveDate) -> Option<Fixing> {
        self.fixings.range(..=date).next_back().map(|(_, f)| *f)
    }

    pub fn between(&self, from: NaiveDate, to: NaiveDate) -> impl Iterator<Item = (&NaiveDate, &Fixing)> {
        self.fixings.range(from..=to)
    }

    /// Annualised drift and volatility of ln(USD/PLN), from business-day log returns in the
    /// `lookback_days` calendar days ending at `as_of`. Annualised with 252 trading days.
    pub fn usd_pln_stats(&self, as_of: NaiveDate, lookback_days: u64) -> Option<SeriesStats> {
        let from = as_of.checked_sub_days(Days::new(lookback_days))?;
        let values: Vec<f64> = self.between(from, as_of).map(|(_, f)| f.usd_pln).collect();
        if values.len() < 20 {
            return None;
        }
        let returns: Vec<f64> = values.windows(2).map(|w| (w[1] / w[0]).ln()).collect();
        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let vol = (var * 252.0).sqrt();
        let last = *values.last()?;
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let below = values.iter().filter(|v| **v <= last).count() as f64;
        Some(SeriesStats {
            lookback_days,
            observations: values.len(),
            vol_annual: vol,
            // GBM drift mu, so that E[S_t] = S_0 e^{mu t}: mean log-return + vol^2/2.
            drift_annual: mean * 252.0 + 0.5 * vol * vol,
            min,
            max,
            last,
            percentile_of_last: below / values.len() as f64,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SeriesStats {
    pub lookback_days: u64,
    pub observations: usize,
    pub vol_annual: f64,
    pub drift_annual: f64,
    pub min: f64,
    pub max: f64,
    pub last: f64,
    /// Share of observations in the window at or below the latest value.
    pub percentile_of_last: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn forward_fills_weekends() {
        let mut m = BTreeMap::new();
        m.insert(d(2026, 10, 16), Fixing { usd_pln: 3.88, usd_eur: 0.89 });
        m.insert(d(2026, 10, 19), Fixing { usd_pln: 3.90, usd_eur: 0.89 });
        let s = RateSeries::new(m);
        assert_eq!(s.on(d(2026, 10, 18)).unwrap().usd_pln, 3.88);
        assert_eq!(s.on(d(2026, 10, 19)).unwrap().usd_pln, 3.90);
        assert!(s.on(d(2026, 10, 15)).is_none());
    }
}
