//! Forward-looking analysis: "convert USD now and earn PLN interest" vs
//! "keep earning USD interest and convert when USD/PLN reaches a target".
//!
//! All values are expressed in PLN at the end of the horizon, so every strategy is compared
//! on the same date. Conversion cost is a proportional spread applied on each conversion.

use chrono::NaiveDate;
use rand::{Rng, SeedableRng, rngs::StdRng};
use rand_distr::StandardNormal;
use serde::Serialize;

use super::savings::SavingsTerms;
use super::stats::{Gbm, quantile_sorted};

#[derive(Debug, Clone)]
pub struct AnalysisInput {
    pub start: NaiveDate,
    pub amount_usd: f64,
    /// PLN per USD now.
    pub spot: f64,
    /// EUR per USD now.
    pub usd_eur: f64,
    /// PLN per USD you are waiting for.
    pub target: f64,
    /// Deadline: if the target isn't hit by then you convert at whatever the rate is.
    pub horizon_days: usize,
    pub spread_pct: f64,
    pub pln: SavingsTerms,
    pub usd: SavingsTerms,
    pub eur: SavingsTerms,
    pub gbm: Gbm,
    pub subjective_prob: Option<f64>,
    pub paths: usize,
    pub seed: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub headline: Headline,
    pub probability: Probability,
    pub monte_carlo: MonteCarlo,
    pub decision: Decision,
    /// One point per day 0..=horizon.
    pub curves: Vec<CurvePoint>,
    pub sensitivity: Sensitivity,
}

#[derive(Debug, Clone, Serialize)]
pub struct Headline {
    pub pln_now: f64,
    pub pln_at_target: f64,
    /// Extra PLN from the better rate alone, ignoring interest.
    pub fx_gain_pln: f64,
    pub pln_interest_per_day: f64,
    pub usd_interest_per_day_pln: f64,
    /// How many days of PLN-account interest the FX gain is worth.
    pub fx_gain_in_pln_interest_days: f64,
    /// Same, but net of the USD interest you keep earning while you wait.
    /// None when USD pays at least as much as PLN — then waiting never costs carry.
    pub breakeven_wait_days: Option<f64>,
    /// Required move in percent and in "pips" (0.0001 PLN).
    pub required_move_pct: f64,
    pub required_move_pips: f64,
    /// Value at horizon of each static strategy.
    pub convert_now_at_horizon: f64,
    pub hold_usd_unchanged_rate_at_horizon: f64,
    pub eur_route_unchanged_rate_at_horizon: f64,
    pub carry_cost_per_day_pln: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Probability {
    pub vol_annual: f64,
    pub drift_annual: f64,
    /// Closed-form GBM probability of touching the target before the deadline.
    pub model_hit_prob: f64,
    pub subjective_prob: Option<f64>,
    /// Expected 1-sigma range of USD/PLN at the deadline.
    pub one_sigma_low: f64,
    pub one_sigma_high: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonteCarlo {
    pub paths: usize,
    pub hit_rate: f64,
    pub mean_days_to_hit: Option<f64>,
    pub mean_value: f64,
    pub mean_value_if_hit: Option<f64>,
    pub mean_value_if_miss: Option<f64>,
    pub prob_beats_convert_now: f64,
    pub p05: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p95: f64,
    /// (strategy value - convert-now value) at horizon by percentile of simulated paths, PLN.
    /// A quantile curve rather than a histogram: the "hit" outcomes pile up at one value and
    /// would flatten every other bin.
    pub edge_quantiles: Vec<QuantilePoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuantilePoint {
    pub q: f64,
    pub edge_pln: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Decision {
    /// Probability of hitting the target that makes waiting break even with converting now,
    /// given the model's conditional outcomes if hit vs. not hit.
    pub required_prob: Option<f64>,
    /// Expected edge of waiting vs converting now, PLN, using the model's probability.
    pub model_edge_pln: f64,
    /// Same, with your probability replacing the model's (conditional outcomes stay modelled).
    pub subjective_edge_pln: Option<f64>,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Wait,
    ConvertNow,
    Toss,
}

#[derive(Debug, Clone, Serialize)]
pub struct CurvePoint {
    pub day: usize,
    pub convert_now: f64,
    pub hold_usd_unchanged: f64,
    /// Value today-forward if the target is hit exactly on this day and converted.
    pub hold_usd_at_target: f64,
    pub eur_route_unchanged: f64,
    /// USD/PLN needed on this day to equal having converted at day 0.
    pub breakeven_rate: f64,
    pub rate_p05: f64,
    pub rate_p25: f64,
    pub rate_p50: f64,
    pub rate_p75: f64,
    pub rate_p95: f64,
    pub hit_prob_by_day: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sensitivity {
    pub targets: Vec<f64>,
    pub hit_days: Vec<usize>,
    /// edge_pln[i][j]: value at horizon minus convert-now value, if `targets[i]` is hit on
    /// `hit_days[j]`.
    pub edge_pln: Vec<Vec<f64>>,
}

pub fn analyze(input: &AnalysisInput) -> Analysis {
    let h = input.horizon_days;
    let keep = 1.0 - input.spread_pct / 100.0;
    let gp = input.pln.growth_path(input.start, h);
    let gu = input.usd.growth_path(input.start, h);
    let ge = input.eur.growth_path(input.start, h);
    let a = input.amount_usd;
    let s0 = input.spot;
    let k = input.target;
    let eur_pln0 = s0 / input.usd_eur;

    // Value at horizon of converting on day t at rate `rate`.
    let convert_on = |t: usize, rate: f64| a * gu[t] * rate * keep * gp[h] / gp[t];
    let convert_now_h = convert_on(0, s0);

    let headline = headline(input, &gu, &ge, keep, convert_now_h);
    let years = h as f64 / 365.0;
    let model_hit = input.gbm.prob_hit_above(s0, k, years);
    let probability = Probability {
        vol_annual: input.gbm.vol,
        drift_annual: input.gbm.drift,
        model_hit_prob: model_hit,
        subjective_prob: input.subjective_prob,
        one_sigma_low: input.gbm.quantile(s0, years, 0.158_655),
        one_sigma_high: input.gbm.quantile(s0, years, 0.841_345),
    };
    let monte_carlo = simulate(input, &convert_on, convert_now_h);
    let decision = decide(&monte_carlo, input.subjective_prob, convert_now_h);

    let curves = (0..=h)
        .map(|t| {
            let yt = t as f64 / 365.0;
            CurvePoint {
                day: t,
                convert_now: a * s0 * keep * gp[t],
                hold_usd_unchanged: a * gu[t] * s0 * keep,
                hold_usd_at_target: a * gu[t] * k * keep,
                eur_route_unchanged: a * input.usd_eur * keep * ge[t] * eur_pln0 * keep,
                breakeven_rate: s0 * gp[t] / gu[t],
                rate_p05: input.gbm.quantile(s0, yt, 0.05),
                rate_p25: input.gbm.quantile(s0, yt, 0.25),
                rate_p50: input.gbm.quantile(s0, yt, 0.50),
                rate_p75: input.gbm.quantile(s0, yt, 0.75),
                rate_p95: input.gbm.quantile(s0, yt, 0.95),
                hit_prob_by_day: input.gbm.prob_hit_above(s0, k, yt),
            }
        })
        .collect();

    let sensitivity = sensitivity(input, &convert_on, convert_now_h);
    Analysis { headline, probability, monte_carlo, decision, curves, sensitivity }
}

fn headline(input: &AnalysisInput, gu: &[f64], ge: &[f64], keep: f64, convert_now_h: f64) -> Headline {
    let h = input.horizon_days;
    let a = input.amount_usd;
    let (s0, k) = (input.spot, input.target);
    let pln_now = a * s0 * keep;
    let pln_at_target = a * k * keep;
    let dp = input.pln.net_daily_rate(input.start);
    let du = input.usd.net_daily_rate(input.start);
    let ratio = (k / s0).ln();
    let fx_days = if dp > 0.0 { ratio / (1.0 + dp).ln() } else { f64::INFINITY };
    let carry = ((1.0 + dp) / (1.0 + du)).ln();
    let breakeven_wait_days = (carry > 0.0).then(|| ratio / carry).filter(|d| *d >= 0.0);
    let eur_pln0 = s0 / input.usd_eur;
    Headline {
        pln_now,
        pln_at_target,
        fx_gain_pln: pln_at_target - pln_now,
        pln_interest_per_day: pln_now * dp,
        usd_interest_per_day_pln: a * du * s0 * keep,
        fx_gain_in_pln_interest_days: fx_days,
        breakeven_wait_days,
        required_move_pct: (k / s0 - 1.0) * 100.0,
        required_move_pips: (k - s0) * 10_000.0,
        convert_now_at_horizon: convert_now_h,
        hold_usd_unchanged_rate_at_horizon: a * gu[h] * s0 * keep,
        eur_route_unchanged_rate_at_horizon: a * input.usd_eur * keep * ge[h] * eur_pln0 * keep,
        carry_cost_per_day_pln: pln_now * (dp - du),
    }
}

/// Simulate the "limit order with deadline" strategy on daily GBM steps.
/// Fills at the target on the first day the simulated fixing reaches it (a resting order
/// fills at its price, not at the overshoot).
fn simulate(input: &AnalysisInput, convert_on: &impl Fn(usize, f64) -> f64, convert_now_h: f64) -> MonteCarlo {
    let h = input.horizon_days;
    let dt = 1.0 / 365.0;
    let nu = input.gbm.drift - 0.5 * input.gbm.vol * input.gbm.vol;
    let step_mean = nu * dt;
    let step_sd = input.gbm.vol * dt.sqrt();
    let mut rng = StdRng::seed_from_u64(input.seed);

    let mut values = Vec::with_capacity(input.paths);
    let (mut hits, mut hit_days_sum, mut hit_sum, mut miss_sum) = (0usize, 0usize, 0.0, 0.0);
    for _ in 0..input.paths {
        let mut s = input.spot;
        let mut outcome = None;
        if s >= input.target {
            outcome = Some(0);
        } else {
            for t in 1..=h {
                let z: f64 = rng.sample(StandardNormal);
                s *= (step_mean + step_sd * z).exp();
                if s >= input.target {
                    outcome = Some(t);
                    break;
                }
            }
        }
        let v = match outcome {
            Some(t) => {
                hits += 1;
                hit_days_sum += t;
                let v = convert_on(t, input.target);
                hit_sum += v;
                v
            }
            None => {
                let v = convert_on(h, s);
                miss_sum += v;
                v
            }
        };
        values.push(v);
    }

    let n = values.len().max(1) as f64;
    let misses = input.paths - hits;
    let beats = values.iter().filter(|v| **v > convert_now_h).count() as f64;
    let mean_value = values.iter().sum::<f64>() / n;
    let mut sorted = values.clone();
    sorted.sort_by(f64::total_cmp);
    let edges: Vec<f64> = sorted.iter().map(|v| v - convert_now_h).collect();
    MonteCarlo {
        paths: input.paths,
        hit_rate: hits as f64 / n,
        mean_days_to_hit: (hits > 0).then(|| hit_days_sum as f64 / hits as f64),
        mean_value,
        mean_value_if_hit: (hits > 0).then(|| hit_sum / hits as f64),
        mean_value_if_miss: (misses > 0).then(|| miss_sum / misses as f64),
        prob_beats_convert_now: beats / n,
        p05: quantile_sorted(&sorted, 0.05),
        p25: quantile_sorted(&sorted, 0.25),
        p50: quantile_sorted(&sorted, 0.50),
        p75: quantile_sorted(&sorted, 0.75),
        p95: quantile_sorted(&sorted, 0.95),
        edge_quantiles: (0..=100).map(|i| {
            let q = i as f64 / 100.0;
            QuantilePoint { q, edge_pln: quantile_sorted(&edges, q) }
        }).collect(),
    }
}

fn decide(mc: &MonteCarlo, subjective: Option<f64>, convert_now_h: f64) -> Decision {
    let required_prob = match (mc.mean_value_if_hit, mc.mean_value_if_miss) {
        (Some(hit), Some(miss)) if hit > miss => Some(((convert_now_h - miss) / (hit - miss)).clamp(0.0, 1.0)),
        (Some(hit), None) => Some(if hit >= convert_now_h { 0.0 } else { 1.0 }),
        (None, Some(miss)) => Some(if miss >= convert_now_h { 0.0 } else { 1.0 }),
        _ => None,
    };
    let expected = |p: f64| match (mc.mean_value_if_hit, mc.mean_value_if_miss) {
        (Some(hit), Some(miss)) => p * hit + (1.0 - p) * miss - convert_now_h,
        _ => mc.mean_value - convert_now_h,
    };
    let model_edge_pln = mc.mean_value - convert_now_h;
    let subjective_edge_pln = subjective.map(expected);
    let deciding = subjective_edge_pln.unwrap_or(model_edge_pln);
    let tolerance = convert_now_h * 1e-4;
    let verdict = if deciding > tolerance {
        Verdict::Wait
    } else if deciding < -tolerance {
        Verdict::ConvertNow
    } else {
        Verdict::Toss
    };
    Decision { required_prob, model_edge_pln, subjective_edge_pln, verdict }
}

fn sensitivity(input: &AnalysisInput, convert_on: &impl Fn(usize, f64) -> f64, convert_now_h: f64) -> Sensitivity {
    let base = (input.spot * 100.0).round() / 100.0;
    let mut targets: Vec<f64> = (-2..=12).map(|i| ((base + i as f64 * 0.01) * 10_000.0).round() / 10_000.0).collect();
    if !targets.iter().any(|t| (t - input.target).abs() < 1e-9) {
        targets.push(input.target);
        targets.sort_by(f64::total_cmp);
    }
    let h = input.horizon_days;
    let mut hit_days: Vec<usize> = [0, 7, 14, 30, 45, 60, 90, 120, 180, 270, 365, 540, 730]
        .into_iter()
        .filter(|d| *d < h)
        .collect();
    hit_days.push(h);
    let edge_pln = targets
        .iter()
        .map(|k| hit_days.iter().map(|t| convert_on(*t, *k) - convert_now_h).collect())
        .collect();
    Sensitivity { targets, hit_days, edge_pln }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::savings::Compounding;
    use approx::assert_relative_eq;

    fn input() -> AnalysisInput {
        AnalysisInput {
            start: NaiveDate::from_ymd_opt(2026, 10, 18).unwrap(),
            amount_usd: 10_000.0,
            spot: 3.89,
            usd_eur: 0.89,
            target: 3.93,
            horizon_days: 90,
            spread_pct: 0.0,
            pln: SavingsTerms::constant(5.0, Compounding::Daily, 19.0),
            usd: SavingsTerms::constant(3.0, Compounding::Daily, 19.0),
            eur: SavingsTerms::constant(2.0, Compounding::Daily, 19.0),
            gbm: Gbm { drift: 0.0, vol: 0.09 },
            subjective_prob: Some(0.6),
            paths: 4000,
            seed: 7,
        }
    }

    #[test]
    fn fx_gain_equals_amount_times_rate_difference() {
        let r = analyze(&input());
        assert_relative_eq!(r.headline.fx_gain_pln, 10_000.0 * 0.04, epsilon = 1e-9);
    }

    #[test]
    fn fx_gain_in_interest_days_matches_compounding() {
        let r = analyze(&input());
        let dp: f64 = 0.05 * 0.81 / 365.0;
        let days = r.headline.fx_gain_in_pln_interest_days;
        assert_relative_eq!(3.89 * (1.0 + dp).powf(days), 3.93, epsilon = 1e-9);
        // ~1% move at ~4% net p.a. is roughly three months of interest.
        assert!(days > 85.0 && days < 100.0, "days={days}");
    }

    #[test]
    fn breakeven_wait_accounts_for_usd_carry() {
        let r = analyze(&input());
        let bw = r.headline.breakeven_wait_days.unwrap();
        assert!(bw > r.headline.fx_gain_in_pln_interest_days);
        // At break-even, hitting the target on that day equals converting at day 0.
        let dp: f64 = 0.05 * 0.81 / 365.0;
        let du: f64 = 0.03 * 0.81 / 365.0;
        assert_relative_eq!(3.93 * (1.0 + du).powf(bw), 3.89 * (1.0 + dp).powf(bw), epsilon = 1e-9);
    }

    #[test]
    fn no_breakeven_when_usd_pays_more() {
        let mut i = input();
        i.usd = SavingsTerms::constant(6.0, Compounding::Daily, 19.0);
        assert!(analyze(&i).headline.breakeven_wait_days.is_none());
    }

    #[test]
    fn monte_carlo_hit_rate_tracks_closed_form() {
        let r = analyze(&input());
        // Daily monitoring hits slightly less often than continuous monitoring.
        let diff = r.probability.model_hit_prob - r.monte_carlo.hit_rate;
        assert!(diff > -0.03 && diff < 0.08, "model={} mc={}", r.probability.model_hit_prob, r.monte_carlo.hit_rate);
    }

    #[test]
    fn required_probability_breaks_even() {
        let r = analyze(&input());
        let p = r.decision.required_prob.unwrap();
        let hit = r.monte_carlo.mean_value_if_hit.unwrap();
        let miss = r.monte_carlo.mean_value_if_miss.unwrap();
        assert_relative_eq!(p * hit + (1.0 - p) * miss, r.headline.convert_now_at_horizon, epsilon = 1e-6);
    }

    #[test]
    fn target_at_spot_is_immediate_hit() {
        let mut i = input();
        i.target = i.spot;
        let r = analyze(&i);
        assert_eq!(r.monte_carlo.hit_rate, 1.0);
        assert_eq!(r.probability.model_hit_prob, 1.0);
    }

    #[test]
    fn breakeven_rate_curve_starts_at_spot_and_rises_with_positive_carry() {
        let r = analyze(&input());
        assert_relative_eq!(r.curves[0].breakeven_rate, 3.89, epsilon = 1e-12);
        assert!(r.curves[90].breakeven_rate > r.curves[30].breakeven_rate);
    }
}
