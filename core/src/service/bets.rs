use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::analyze::{AnalyzeRequest, build};
use super::{non_empty, positive};
use crate::domain::analysis;
use crate::domain::calibration::{Calibration, ResolvedBet, calibrate};
use crate::domain::fx::{Fixing, RateSeries};
use crate::domain::tracking::{BetPlan, Conversion, Outcome, Summary, Tracking, track};
use crate::error::{Error, Result, bad};
use crate::model::{Account, Bet, ConversionRecord, NewBet};

#[derive(Deserialize)]
pub struct CreateBet {
    title: String,
    #[serde(default)]
    notes: String,
    analysis: AnalyzeRequest,
    /// Recorded after the fact; its subjective probability is excluded from calibration.
    #[serde(default)]
    retrospective: bool,
    /// Exchanges already made, recorded together with the bet (typical for retrospective bets).
    #[serde(default)]
    conversions: Vec<NewConversion>,
}

/// Validates a new bet and snapshots its analysis. CPU-bound (Monte Carlo), like `analyze`.
/// The caller stores the bet and its conversions together.
pub fn prepare(body: CreateBet, series: &RateSeries, accounts: &[Account], today: NaiveDate) -> Result<(NewBet, Vec<NewConversion>)> {
    let title = non_empty("title", &body.title)?;
    let req = body.analysis;
    let (Some(pln_id), Some(usd_id), Some(eur_id)) = (req.pln_account_id, req.usd_account_id, req.eur_account_id) else {
        return Err(bad("a bet must reference pln_account_id, usd_account_id and eur_account_id so it can be replayed"));
    };
    // A tracked bet is valued with the accounts' real rate history; explicit overrides would
    // make the snapshot and the replay disagree.
    let req = AnalyzeRequest { pln: None, usd: None, eur: None, ..req };
    let (input, inputs) = build(&req, series, accounts, today)?;
    let conversions = body.conversions.iter().map(|c| c.validate(inputs.start_date, today)).collect::<Result<Vec<_>>>()?;
    let result = analysis::analyze(&input);

    let snapshot = json!({
        "request": req,
        "inputs": inputs,
        "headline": result.headline,
        "probability": result.probability,
        "decision": result.decision,
        "monte_carlo": {
            "hit_rate": result.monte_carlo.hit_rate,
            "mean_value": result.monte_carlo.mean_value,
            "prob_beats_convert_now": result.monte_carlo.prob_beats_convert_now,
            "p05": result.monte_carlo.p05,
            "p50": result.monte_carlo.p50,
            "p95": result.monte_carlo.p95,
        },
    });
    let bet = NewBet {
        title,
        notes: body.notes,
        amount_usd: req.amount_usd,
        start_date: inputs.start_date,
        deadline: inputs.deadline,
        spot_usd_pln: inputs.spot,
        spot_usd_eur: inputs.usd_eur,
        target_rate: req.target,
        spread_pct: req.spread_pct,
        subjective_prob: req.subjective_prob,
        model_prob: result.probability.model_hit_prob,
        pln_account_id: pln_id,
        usd_account_id: usd_id,
        eur_account_id: eur_id,
        snapshot,
        retrospective: body.retrospective,
    };
    Ok((bet, conversions))
}

#[derive(Deserialize)]
pub struct UpdateBet {
    title: String,
    pub notes: String,
}

impl UpdateBet {
    pub fn title(&self) -> Result<String> {
        non_empty("title", &self.title)
    }
}

#[derive(Deserialize)]
pub struct NewConversion {
    pub date: NaiveDate,
    pub usd_amount: f64,
    /// PLN received per USD (effective, after spread).
    pub rate: f64,
    #[serde(default)]
    pub note: String,
}

impl NewConversion {
    pub fn validate(&self, start: NaiveDate, today: NaiveDate) -> Result<NewConversion> {
        if self.date < start || self.date > today {
            return Err(bad(format!("conversion on {} must be between the bet start ({start}) and today", self.date)));
        }
        let usd_amount = positive("usd_amount", self.usd_amount)?;
        let rate = positive("rate", self.rate)?;
        if !(0.5..=20.0).contains(&rate) {
            return Err(bad("rate looks wrong: expected PLN per 1 USD"));
        }
        Ok(NewConversion { date: self.date, usd_amount, rate, note: self.note.trim().to_string() })
    }
}

#[derive(Serialize)]
pub struct BetListItem {
    bet: Bet,
    status: BetStatus,
    summary: Summary,
    outcome: Outcome,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum BetStatus {
    Open,
    Hit,
    Missed,
}

#[derive(Serialize)]
pub struct BetDetail {
    bet: Bet,
    status: BetStatus,
    conversions: Vec<ConversionRecord>,
    accounts: Vec<Account>,
    tracking: Tracking,
}

#[derive(Serialize)]
pub struct CalibrationResponse {
    calibration: Calibration,
    points: Vec<CalibrationPoint>,
}

#[derive(Serialize)]
pub struct CalibrationPoint {
    bet_id: i64,
    title: String,
    start_date: NaiveDate,
    subjective_prob: Option<f64>,
    model_prob: f64,
    target_hit: bool,
    edge_vs_day0_pln: f64,
    retrospective: bool,
}

/// Everything a bet replay needs besides the bet itself.
pub struct Ctx {
    series: RateSeries,
    accounts: HashMap<i64, Account>,
    today: NaiveDate,
}

impl Ctx {
    pub fn new(series: RateSeries, accounts: Vec<Account>, today: NaiveDate) -> Self {
        Self { series, accounts: accounts.into_iter().map(|a| (a.id, a)).collect(), today }
    }

    fn track(&self, bet: &Bet, conversions: &[ConversionRecord]) -> Result<Tracking> {
        let acc = |id: i64| self.accounts.get(&id).ok_or_else(|| Error::Internal(format!("bet={} account={id} missing", bet.id)));
        let plan = BetPlan {
            start: bet.start_date,
            deadline: bet.deadline,
            amount_usd: bet.amount_usd,
            start_fixing: Fixing { usd_pln: bet.spot_usd_pln, usd_eur: bet.spot_usd_eur },
            target: bet.target_rate,
            spread_pct: bet.spread_pct,
            pln: acc(bet.pln_account_id)?.terms(),
            usd: acc(bet.usd_account_id)?.terms(),
            eur: acc(bet.eur_account_id)?.terms(),
            subjective_prob: bet.subjective_prob,
            model_prob: bet.model_prob,
        };
        let conv: Vec<Conversion> =
            conversions.iter().map(|c| Conversion { date: c.date, usd_amount: c.usd_amount, rate: c.rate }).collect();
        Ok(track(&plan, &conv, &self.series, self.today.max(bet.start_date)))
    }

    /// `conversions` is keyed by bet id; bets without an entry have none.
    pub fn list(&self, bets: Vec<Bet>, mut conversions: HashMap<i64, Vec<ConversionRecord>>) -> Result<Vec<BetListItem>> {
        bets.into_iter()
            .map(|bet| {
                let conv = conversions.remove(&bet.id).unwrap_or_default();
                let t = self.track(&bet, &conv)?;
                Ok(BetListItem { status: status(&t.outcome), summary: t.summary, outcome: t.outcome, bet })
            })
            .collect()
    }

    pub fn detail(&self, bet: Bet, conversions: Vec<ConversionRecord>) -> Result<BetDetail> {
        let tracking = self.track(&bet, &conversions)?;
        let ids = [bet.pln_account_id, bet.usd_account_id, bet.eur_account_id];
        let accounts = ids.iter().filter_map(|i| self.accounts.get(i).cloned()).collect();
        Ok(BetDetail { status: status(&tracking.outcome), bet, conversions, accounts, tracking })
    }

    pub fn calibration(&self, bets: Vec<Bet>, mut conversions: HashMap<i64, Vec<ConversionRecord>>) -> Result<CalibrationResponse> {
        let mut points = Vec::new();
        for bet in bets {
            let conv = conversions.remove(&bet.id).unwrap_or_default();
            let t = self.track(&bet, &conv)?;
            if t.outcome.resolved {
                points.push(CalibrationPoint {
                    bet_id: bet.id,
                    title: bet.title,
                    start_date: bet.start_date,
                    subjective_prob: bet.subjective_prob,
                    model_prob: bet.model_prob,
                    target_hit: t.outcome.target_hit,
                    edge_vs_day0_pln: t.summary.edge_vs_day0_pln,
                    retrospective: bet.retrospective,
                });
            }
        }
        let resolved: Vec<ResolvedBet> = points
            .iter()
            .map(|p| ResolvedBet {
                // A probability written down after the outcome is known isn't a forecast.
                subjective_prob: if p.retrospective { None } else { p.subjective_prob },
                model_prob: p.model_prob,
                target_hit: p.target_hit,
                edge_vs_day0_pln: p.edge_vs_day0_pln,
            })
            .collect();
        Ok(CalibrationResponse { calibration: calibrate(&resolved), points })
    }
}

fn status(o: &Outcome) -> BetStatus {
    match (o.resolved, o.target_hit) {
        (false, _) => BetStatus::Open,
        (true, true) => BetStatus::Hit,
        (true, false) => BetStatus::Missed,
    }
}
