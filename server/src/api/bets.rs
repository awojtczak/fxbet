use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::analyze::{AnalyzeRequest, build};
use super::{AppState, non_empty, positive};
use crate::db::{self, Account, Bet, ConversionRecord, NewBet};
use crate::domain::analysis;
use crate::domain::calibration::{Calibration, ResolvedBet, calibrate};
use crate::domain::fx::{Fixing, RateSeries};
use crate::domain::tracking::{BetPlan, Conversion, Outcome, Summary, Tracking, track};
use crate::error::{AppError, AppResult, bad};

#[derive(Deserialize)]
pub struct CreateBet {
    title: String,
    #[serde(default)]
    notes: String,
    analysis: AnalyzeRequest,
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

pub async fn create(State(st): State<AppState>, Json(body): Json<CreateBet>) -> AppResult<(StatusCode, Json<BetDetail>)> {
    let title = non_empty("title", &body.title)?;
    let req = body.analysis;
    let (Some(pln_id), Some(usd_id), Some(eur_id)) = (req.pln_account_id, req.usd_account_id, req.eur_account_id) else {
        return Err(bad("a bet must reference pln_account_id, usd_account_id and eur_account_id so it can be replayed"));
    };
    // A tracked bet is valued with the accounts' real rate history; explicit overrides would
    // make the snapshot and the replay disagree.
    let req = AnalyzeRequest { pln: None, usd: None, eur: None, ..req };
    let series = db::load_series(&st.pool).await?;
    let accounts = db::list_accounts(&st.pool).await?;
    let (input, inputs) = build(&req, &series, &accounts, st.clock.today())?;
    let result = tokio::task::spawn_blocking(move || analysis::analyze(&input))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

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
    let id = db::insert_bet(
        &st.pool,
        &NewBet {
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
        },
    )
    .await?;
    tracing::info!(bet = id, amount_usd = req.amount_usd, target = req.target, "bet created");
    Ok((StatusCode::CREATED, Json(load_detail(&st, id).await?)))
}

pub async fn list(State(st): State<AppState>) -> AppResult<Json<Vec<BetListItem>>> {
    let ctx = Ctx::load(&st).await?;
    let bets = db::list_bets(&st.pool).await?;
    let mut conversions = db::list_all_conversions(&st.pool).await?;
    let items = bets
        .into_iter()
        .map(|bet| {
            let conv = conversions.remove(&bet.id).unwrap_or_default();
            let t = ctx.track(&bet, &conv)?;
            Ok(BetListItem { status: status(&t.outcome), summary: t.summary, outcome: t.outcome, bet })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Json(items))
}

pub async fn detail(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<BetDetail>> {
    Ok(Json(load_detail(&st, id).await?))
}

#[derive(Deserialize)]
pub struct UpdateBet {
    title: String,
    notes: String,
}

pub async fn update(State(st): State<AppState>, Path(id): Path<i64>, Json(body): Json<UpdateBet>) -> AppResult<Json<BetDetail>> {
    db::update_bet_notes(&st.pool, id, &non_empty("title", &body.title)?, &body.notes).await?;
    Ok(Json(load_detail(&st, id).await?))
}

pub async fn remove(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<StatusCode> {
    db::delete_bet(&st.pool, id).await?;
    tracing::info!(bet = id, "bet deleted");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct NewConversion {
    date: NaiveDate,
    usd_amount: f64,
    /// PLN received per USD (effective, after spread).
    rate: f64,
    #[serde(default)]
    note: String,
}

pub async fn add_conversion(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<NewConversion>,
) -> AppResult<(StatusCode, Json<BetDetail>)> {
    let bet = db::get_bet(&st.pool, id).await?;
    if body.date < bet.start_date || body.date > st.clock.today() {
        return Err(bad("conversion date must be between the bet start and today"));
    }
    let amount = positive("usd_amount", body.usd_amount)?;
    let rate = positive("rate", body.rate)?;
    if !(0.5..=20.0).contains(&rate) {
        return Err(bad("rate looks wrong: expected PLN per 1 USD"));
    }
    db::insert_conversion(&st.pool, id, body.date, amount, rate, body.note.trim()).await?;
    tracing::info!(bet = id, usd_amount = amount, rate, date = %body.date, "conversion recorded");
    Ok((StatusCode::CREATED, Json(load_detail(&st, id).await?)))
}

pub async fn remove_conversion(State(st): State<AppState>, Path((id, conversion_id)): Path<(i64, i64)>) -> AppResult<Json<BetDetail>> {
    db::delete_conversion(&st.pool, id, conversion_id).await?;
    Ok(Json(load_detail(&st, id).await?))
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
}

pub async fn calibration(State(st): State<AppState>) -> AppResult<Json<CalibrationResponse>> {
    let ctx = Ctx::load(&st).await?;
    let bets = db::list_bets(&st.pool).await?;
    let mut conversions = db::list_all_conversions(&st.pool).await?;
    let mut points = Vec::new();
    for bet in bets {
        let conv = conversions.remove(&bet.id).unwrap_or_default();
        let t = ctx.track(&bet, &conv)?;
        if t.outcome.resolved {
            points.push(CalibrationPoint {
                bet_id: bet.id,
                title: bet.title,
                start_date: bet.start_date,
                subjective_prob: bet.subjective_prob,
                model_prob: bet.model_prob,
                target_hit: t.outcome.target_hit,
                edge_vs_day0_pln: t.summary.edge_vs_day0_pln,
            });
        }
    }
    let resolved: Vec<ResolvedBet> = points
        .iter()
        .map(|p| ResolvedBet {
            subjective_prob: p.subjective_prob,
            model_prob: p.model_prob,
            target_hit: p.target_hit,
            edge_vs_day0_pln: p.edge_vs_day0_pln,
        })
        .collect();
    Ok(Json(CalibrationResponse { calibration: calibrate(&resolved), points }))
}

// ---------- shared ----------

struct Ctx {
    series: RateSeries,
    accounts: HashMap<i64, Account>,
    today: NaiveDate,
}

impl Ctx {
    async fn load(st: &AppState) -> AppResult<Self> {
        let series = db::load_series(&st.pool).await?;
        let accounts = db::list_accounts(&st.pool).await?.into_iter().map(|a| (a.id, a)).collect();
        Ok(Self { series, accounts, today: st.clock.today() })
    }

    fn track(&self, bet: &Bet, conversions: &[ConversionRecord]) -> AppResult<Tracking> {
        let acc = |id: i64| self.accounts.get(&id).ok_or_else(|| AppError::Internal(anyhow::anyhow!("bet={} account={id} missing", bet.id)));
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
}

fn status(o: &Outcome) -> BetStatus {
    match (o.resolved, o.target_hit) {
        (false, _) => BetStatus::Open,
        (true, true) => BetStatus::Hit,
        (true, false) => BetStatus::Missed,
    }
}

async fn load_detail(st: &AppState, id: i64) -> AppResult<BetDetail> {
    let bet = db::get_bet(&st.pool, id).await?;
    let conversions = db::list_conversions(&st.pool, id).await?;
    let ctx = Ctx::load(st).await?;
    let tracking = ctx.track(&bet, &conversions)?;
    let ids = [bet.pln_account_id, bet.usd_account_id, bet.eur_account_id];
    let accounts = ids.iter().filter_map(|i| ctx.accounts.get(i).cloned()).collect();
    Ok(BetDetail { status: status(&tracking.outcome), bet, conversions, accounts, tracking })
}
