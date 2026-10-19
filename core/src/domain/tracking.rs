//! Replays a recorded bet against realised fixings and account rates, and values the
//! alternative paths you could have taken with the same USD amount.

use chrono::{Days, NaiveDate};
use serde::Serialize;

use super::fx::{Fixing, RateSeries};
use super::savings::SavingsTerms;

#[derive(Debug, Clone)]
pub struct BetPlan {
    pub start: NaiveDate,
    pub deadline: NaiveDate,
    pub amount_usd: f64,
    /// Fixing the bet was analysed against; used for day 0 so the replay starts where you did.
    pub start_fixing: Fixing,
    pub target: f64,
    pub spread_pct: f64,
    pub pln: SavingsTerms,
    pub usd: SavingsTerms,
    pub eur: SavingsTerms,
    pub subjective_prob: Option<f64>,
    pub model_prob: f64,
}

#[derive(Debug, Clone)]
pub struct Conversion {
    pub date: NaiveDate,
    pub usd_amount: f64,
    /// PLN actually received per USD, after your bank's spread.
    pub rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tracking {
    pub as_of: NaiveDate,
    pub days: Vec<DayValue>,
    /// Final value (at `as_of`) had you converted everything on that day.
    pub conversion_days: Vec<ConversionDay>,
    pub summary: Summary,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayValue {
    pub date: NaiveDate,
    pub usd_pln: f64,
    pub eur_pln: f64,
    pub actual: f64,
    pub convert_day0: f64,
    pub hold_usd: f64,
    pub eur_route: f64,
    pub target_order: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversionDay {
    pub date: NaiveDate,
    pub usd_pln: f64,
    pub final_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub actual: f64,
    pub convert_day0: f64,
    pub hold_usd: f64,
    pub eur_route: f64,
    pub target_order: f64,
    pub edge_vs_day0_pln: f64,
    /// Edge expressed as days of today's PLN-account interest on the day-0 baseline.
    pub edge_in_pln_interest_days: Option<f64>,
    pub best_day: Option<ConversionDay>,
    pub worst_day: Option<ConversionDay>,
    /// Share of possible single-day conversions that the actual path beats.
    pub actual_beats_share: f64,
    pub regret_vs_best_pln: Option<f64>,
    pub usd_remaining: f64,
    pub converted_usd: f64,
    pub avg_conversion_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub resolved: bool,
    pub target_hit: bool,
    pub first_hit_date: Option<NaiveDate>,
    pub max_rate_in_window: f64,
    pub min_rate_in_window: f64,
    pub brier_subjective: Option<f64>,
    pub brier_model: Option<f64>,
}

/// `as_of` must be on or after `plan.start`; conversions are assumed validated (dated within
/// [start, as_of], positive amounts and rates).
pub fn track(plan: &BetPlan, conversions: &[Conversion], series: &RateSeries, as_of: NaiveDate) -> Tracking {
    let n = (as_of - plan.start).num_days().max(0) as usize;
    let keep = 1.0 - plan.spread_pct / 100.0;
    let a = plan.amount_usd;
    let fix: Vec<Fixing> = (0..=n)
        .map(|i| match i {
            0 => plan.start_fixing,
            _ => series.on(plan.start + Days::new(i as u64)).unwrap_or(plan.start_fixing),
        })
        .collect();
    let gp = plan.pln.growth_path(plan.start, n);
    let gu = plan.usd.growth_path(plan.start, n);
    let ge = plan.eur.growth_path(plan.start, n);
    let deadline_idx = ((plan.deadline - plan.start).num_days().max(0) as usize).min(n);
    let deadline_passed = as_of >= plan.deadline;

    let window = &fix[..=deadline_idx];
    let first_hit = window.iter().position(|f| f.usd_pln >= plan.target);
    let target_fill = first_hit.map(|j| (j, plan.target)).or_else(|| deadline_passed.then(|| (deadline_idx, fix[deadline_idx].usd_pln)));

    let mut sorted_conv: Vec<&Conversion> = conversions.iter().collect();
    sorted_conv.sort_by_key(|c| c.date);
    let mut next_conv = 0;
    let (mut usd_bal, mut pln_bal) = (a, 0.0);
    let (mut converted_usd, mut received_pln) = (0.0, 0.0);
    let eur0 = a * plan.start_fixing.usd_eur * keep;

    let mut days = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let date = plan.start + Days::new(i as u64);
        if i > 0 {
            usd_bal *= gu[i] / gu[i - 1];
            pln_bal *= gp[i] / gp[i - 1];
        }
        while next_conv < sorted_conv.len() && sorted_conv[next_conv].date <= date {
            let c = sorted_conv[next_conv];
            let sold = c.usd_amount.min(usd_bal);
            usd_bal -= sold;
            pln_bal += sold * c.rate;
            converted_usd += sold;
            received_pln += sold * c.rate;
            next_conv += 1;
        }
        let f = fix[i];
        let target_order = match target_fill {
            Some((j, rate)) if i >= j => a * gu[j] * rate * keep * gp[i] / gp[j],
            _ => a * gu[i] * f.usd_pln * keep,
        };
        days.push(DayValue {
            date,
            usd_pln: f.usd_pln,
            eur_pln: f.eur_pln(),
            actual: usd_bal * f.usd_pln * keep + pln_bal,
            convert_day0: a * plan.start_fixing.usd_pln * keep * gp[i],
            hold_usd: a * gu[i] * f.usd_pln * keep,
            eur_route: eur0 * ge[i] * f.eur_pln() * keep,
            target_order,
        });
    }

    let conversion_days: Vec<ConversionDay> = (0..=n)
        .map(|d| ConversionDay {
            date: plan.start + Days::new(d as u64),
            usd_pln: fix[d].usd_pln,
            final_value: a * gu[d] * fix[d].usd_pln * keep * gp[n] / gp[d],
        })
        .collect();

    let last = days.last().expect("days always has day 0");
    let best_day = conversion_days.iter().max_by(|x, y| x.final_value.total_cmp(&y.final_value)).cloned();
    let worst_day = conversion_days.iter().min_by(|x, y| x.final_value.total_cmp(&y.final_value)).cloned();
    let beats = conversion_days.iter().filter(|c| last.actual > c.final_value).count();
    let edge = last.actual - last.convert_day0;
    let daily_interest = last.convert_day0 * plan.pln.net_daily_rate(as_of);
    let summary = Summary {
        actual: last.actual,
        convert_day0: last.convert_day0,
        hold_usd: last.hold_usd,
        eur_route: last.eur_route,
        target_order: last.target_order,
        edge_vs_day0_pln: edge,
        edge_in_pln_interest_days: (daily_interest > 0.0).then(|| edge / daily_interest),
        regret_vs_best_pln: best_day.as_ref().map(|b| b.final_value - last.actual),
        best_day,
        worst_day,
        actual_beats_share: beats as f64 / conversion_days.len().max(1) as f64,
        usd_remaining: usd_bal,
        converted_usd,
        avg_conversion_rate: (converted_usd > 0.0).then(|| received_pln / converted_usd),
    };

    let target_hit = first_hit.is_some();
    let resolved = target_hit || deadline_passed;
    let outcome_value = if target_hit { 1.0 } else { 0.0 };
    let brier = |p: f64| (p - outcome_value).powi(2);
    let outcome = Outcome {
        resolved,
        target_hit,
        first_hit_date: first_hit.map(|j| plan.start + Days::new(j as u64)),
        max_rate_in_window: window.iter().map(|f| f.usd_pln).fold(f64::NEG_INFINITY, f64::max),
        min_rate_in_window: window.iter().map(|f| f.usd_pln).fold(f64::INFINITY, f64::min),
        brier_subjective: if resolved { plan.subjective_prob.map(brier) } else { None },
        brier_model: resolved.then(|| brier(plan.model_prob)),
    };

    Tracking { as_of, days, conversion_days, summary, outcome }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::savings::Compounding;
    use approx::assert_relative_eq;
    use std::collections::BTreeMap;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    fn fx(usd_pln: f64) -> Fixing {
        Fixing { usd_pln, usd_eur: 0.9 }
    }

    fn plan() -> BetPlan {
        BetPlan {
            start: d(1),
            deadline: d(10),
            amount_usd: 1000.0,
            start_fixing: fx(3.89),
            target: 3.93,
            spread_pct: 0.0,
            pln: SavingsTerms::constant(0.0, Compounding::Daily, 0.0),
            usd: SavingsTerms::constant(0.0, Compounding::Daily, 0.0),
            eur: SavingsTerms::constant(0.0, Compounding::Daily, 0.0),
            subjective_prob: Some(0.8),
            model_prob: 0.4,
        }
    }

    fn series() -> RateSeries {
        let mut m = BTreeMap::new();
        m.insert(d(1), fx(3.89));
        m.insert(d(3), fx(3.91));
        m.insert(d(5), fx(3.94));
        m.insert(d(8), fx(3.87));
        RateSeries::new(m)
    }

    #[test]
    fn actual_path_reflects_conversion_at_recorded_rate() {
        let conv = [Conversion { date: d(5), usd_amount: 1000.0, rate: 3.94 }];
        let t = track(&plan(), &conv, &series(), d(9));
        assert_relative_eq!(t.summary.actual, 3940.0, epsilon = 1e-9);
        assert_relative_eq!(t.summary.edge_vs_day0_pln, 50.0, epsilon = 1e-9);
        assert_eq!(t.summary.usd_remaining, 0.0);
    }

    #[test]
    fn unconverted_usd_is_marked_to_market() {
        let t = track(&plan(), &[], &series(), d(9));
        assert_relative_eq!(t.summary.actual, 3870.0, epsilon = 1e-9);
    }

    #[test]
    fn target_order_fills_at_target_on_first_touch() {
        let t = track(&plan(), &[], &series(), d(9));
        assert_eq!(t.outcome.first_hit_date, Some(d(5)));
        assert!(t.outcome.resolved && t.outcome.target_hit);
        assert_relative_eq!(t.summary.target_order, 3930.0, epsilon = 1e-9);
        assert_relative_eq!(t.outcome.brier_subjective.unwrap(), 0.04, epsilon = 1e-12);
    }

    #[test]
    fn best_conversion_day_is_the_peak() {
        let t = track(&plan(), &[], &series(), d(9));
        assert_eq!(t.summary.best_day.unwrap().usd_pln, 3.94);
        assert_eq!(t.summary.worst_day.unwrap().usd_pln, 3.87);
    }

    #[test]
    fn unresolved_before_deadline_without_hit() {
        let mut p = plan();
        p.target = 4.5;
        let t = track(&p, &[], &series(), d(9));
        assert!(!t.outcome.resolved);
        assert!(t.outcome.brier_model.is_none());
    }

    #[test]
    fn interest_accrues_on_converted_pln() {
        let mut p = plan();
        p.pln = SavingsTerms::constant(36.5, Compounding::Daily, 0.0);
        let conv = [Conversion { date: d(1), usd_amount: 1000.0, rate: 3.89 }];
        let t = track(&p, &conv, &series(), d(3));
        assert_relative_eq!(t.summary.actual, 3890.0 * 1.001f64.powi(2), epsilon = 1e-9);
        assert_relative_eq!(t.summary.edge_vs_day0_pln, 0.0, epsilon = 1e-9);
    }
}
