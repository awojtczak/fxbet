//! Savings-account growth with variable rates, capitalization schedule and tax on interest.
//!
//! Interest accrues daily on ACT/365. On each capitalization date the accrued interest,
//! net of tax, is added to the balance (this is how Polish banks apply the 19% "Belka" tax:
//! withheld at every capitalization). Per-capitalization rounding to the cent is ignored.

use chrono::{Datelike, Days, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Compounding {
    Daily,
    Monthly,
}

impl Compounding {
    pub fn as_str(self) -> &'static str {
        match self {
            Compounding::Daily => "daily",
            Compounding::Monthly => "monthly",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "daily" => Some(Compounding::Daily),
            "monthly" => Some(Compounding::Monthly),
            _ => None,
        }
    }
}

/// Rate changes over time. Invariant: non-empty, sorted by date, rates in percent per annum.
#[derive(Debug, Clone)]
pub struct RateSchedule {
    steps: Vec<(NaiveDate, f64)>,
}

impl RateSchedule {
    pub fn constant(annual_rate_pct: f64) -> Self {
        Self { steps: vec![(NaiveDate::MIN, annual_rate_pct)] }
    }

    /// Steps before the first effective date fall back to the first rate, so a schedule
    /// entered "from today" can still value a bet that started a bit earlier.
    pub fn from_steps(mut steps: Vec<(NaiveDate, f64)>) -> Option<Self> {
        if steps.is_empty() {
            return None;
        }
        steps.sort_by_key(|(d, _)| *d);
        Some(Self { steps })
    }

    pub fn rate_on(&self, date: NaiveDate) -> f64 {
        match self.steps.partition_point(|(d, _)| *d <= date) {
            0 => self.steps[0].1,
            i => self.steps[i - 1].1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SavingsTerms {
    pub schedule: RateSchedule,
    pub compounding: Compounding,
    pub tax_pct: f64,
}

impl SavingsTerms {
    pub fn constant(annual_rate_pct: f64, compounding: Compounding, tax_pct: f64) -> Self {
        Self { schedule: RateSchedule::constant(annual_rate_pct), compounding, tax_pct }
    }

    /// Net (after-tax) daily rate at a date, as a fraction.
    pub fn net_daily_rate(&self, date: NaiveDate) -> f64 {
        self.schedule.rate_on(date) / 100.0 * (1.0 - self.tax_pct / 100.0) / 365.0
    }

    /// Value of 1 unit deposited at `start`, observed at `start + i` days for i in 0..=days.
    /// Values include accrued-but-not-yet-capitalized interest, net of tax, so a monthly
    /// account doesn't look like a staircase when compared to a daily one.
    pub fn growth_path(&self, start: NaiveDate, days: usize) -> Vec<f64> {
        let tax_keep = 1.0 - self.tax_pct / 100.0;
        let mut out = Vec::with_capacity(days + 1);
        let mut balance = 1.0;
        let mut accrued = 0.0;
        out.push(balance);
        let mut date = start;
        for _ in 0..days {
            accrued += balance * self.schedule.rate_on(date) / 100.0 / 365.0;
            date = date + Days::new(1);
            let capitalize = match self.compounding {
                Compounding::Daily => true,
                Compounding::Monthly => date.day() == 1,
            };
            if capitalize {
                balance += accrued * tax_keep;
                accrued = 0.0;
            }
            out.push(balance + accrued * tax_keep);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn daily_compounding_matches_closed_form() {
        let terms = SavingsTerms::constant(5.0, Compounding::Daily, 19.0);
        let path = terms.growth_path(d(2026, 1, 1), 365);
        let daily = 0.05 * 0.81 / 365.0;
        assert_relative_eq!(path[365], (1.0f64 + daily).powi(365), epsilon = 1e-12);
    }

    #[test]
    fn monthly_capitalizes_on_first_of_month() {
        let terms = SavingsTerms::constant(12.0, Compounding::Monthly, 0.0);
        let path = terms.growth_path(d(2026, 1, 1), 59);
        // Jan has 31 days of simple interest, then Feb compounds on top of it.
        let jan = 1.0 + 0.12 * 31.0 / 365.0;
        assert_relative_eq!(path[31], jan, epsilon = 1e-12);
        assert_relative_eq!(path[59], jan * (1.0 + 0.12 * 28.0 / 365.0), epsilon = 1e-12);
    }

    #[test]
    fn schedule_applies_latest_step_on_or_before_date() {
        let s = RateSchedule::from_steps(vec![(d(2026, 3, 1), 4.0), (d(2026, 1, 1), 5.0)]).unwrap();
        assert_eq!(s.rate_on(d(2025, 12, 1)), 5.0);
        assert_eq!(s.rate_on(d(2026, 2, 28)), 5.0);
        assert_eq!(s.rate_on(d(2026, 3, 1)), 4.0);
    }
}
