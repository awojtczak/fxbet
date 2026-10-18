use axum::Json;
use axum::extract::State;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::{AppState, pct, positive, prob};
use crate::db::{self, Account};
use crate::domain::analysis::{self, Analysis, AnalysisInput};
use crate::domain::fx::{Currency, RateSeries};
use crate::domain::savings::{Compounding, SavingsTerms};
use crate::domain::stats::Gbm;
use crate::error::{AppError, AppResult, bad};

const MAX_HORIZON_DAYS: u32 = 1095;
const MAX_PATHS: usize = 50_000;

/// Explicit savings terms, used instead of an account's rate schedule for what-if analysis.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TermsOverride {
    pub annual_rate_pct: f64,
    pub compounding: Compounding,
    pub tax_pct: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnalyzeRequest {
    pub amount_usd: f64,
    pub target: f64,
    pub horizon_days: u32,
    /// Defaults to today; a past date replays the analysis as of that day.
    pub start_date: Option<NaiveDate>,
    /// Defaults to the fixing on `start_date`.
    pub spot: Option<f64>,
    pub usd_eur: Option<f64>,
    #[serde(default)]
    pub spread_pct: f64,
    pub pln_account_id: Option<i64>,
    pub usd_account_id: Option<i64>,
    pub eur_account_id: Option<i64>,
    pub pln: Option<TermsOverride>,
    pub usd: Option<TermsOverride>,
    pub eur: Option<TermsOverride>,
    /// Annualised volatility in percent; defaults to realised over `lookback_days`.
    pub vol_pct: Option<f64>,
    /// Annualised drift in percent; defaults to 0 (no view on direction).
    pub drift_pct: Option<f64>,
    pub lookback_days: Option<u64>,
    pub subjective_prob: Option<f64>,
    pub paths: Option<usize>,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedInputs {
    pub start_date: NaiveDate,
    pub deadline: NaiveDate,
    pub spot: f64,
    pub usd_eur: f64,
    pub fixing_date: Option<NaiveDate>,
    pub vol_pct: f64,
    pub vol_source: &'static str,
    pub drift_pct: f64,
    pub pln_rate_pct: f64,
    pub usd_rate_pct: f64,
    pub eur_rate_pct: f64,
}

#[derive(Serialize)]
pub struct AnalyzeResponse {
    pub inputs: ResolvedInputs,
    pub analysis: Analysis,
}

pub async fn analyze(State(st): State<AppState>, Json(req): Json<AnalyzeRequest>) -> AppResult<Json<AnalyzeResponse>> {
    let series = db::load_series(&st.pool).await?;
    let accounts = db::list_accounts(&st.pool).await?;
    let (input, inputs) = build(&req, &series, &accounts, st.clock.today())?;
    let analysis = tokio::task::spawn_blocking(move || analysis::analyze(&input))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(AnalyzeResponse { inputs, analysis }))
}

/// Validates the request and resolves defaults from market data and accounts.
pub fn build(
    req: &AnalyzeRequest,
    series: &RateSeries,
    accounts: &[Account],
    today: NaiveDate,
) -> AppResult<(AnalysisInput, ResolvedInputs)> {
    let start = req.start_date.unwrap_or(today);
    if start > today {
        return Err(bad("start_date cannot be in the future"));
    }
    if req.horizon_days == 0 || req.horizon_days > MAX_HORIZON_DAYS {
        return Err(bad(format!("horizon_days must be in [1, {MAX_HORIZON_DAYS}]")));
    }
    let fixing = series.on(start);
    let fixing_date = series.between(NaiveDate::MIN, start).last().map(|(d, _)| *d);
    let spot = match req.spot {
        Some(s) => positive("spot", s)?,
        None => fixing.map(|f| f.usd_pln).ok_or_else(|| bad("no fixing available for start_date; pass spot"))?,
    };
    let usd_eur = match req.usd_eur {
        Some(s) => positive("usd_eur", s)?,
        None => fixing.map(|f| f.usd_eur).ok_or_else(|| bad("no fixing available for start_date; pass usd_eur"))?,
    };
    let lookback = req.lookback_days.unwrap_or(365);
    if !(30..=3650).contains(&lookback) {
        return Err(bad("lookback_days must be in [30, 3650]"));
    }
    let (vol, vol_source) = match req.vol_pct {
        Some(v) if v.is_finite() && v > 0.0 && v < 200.0 => (v / 100.0, "manual"),
        Some(_) => return Err(bad("vol_pct must be in (0, 200)")),
        None => match series.usd_pln_stats(start, lookback) {
            Some(s) => (s.vol_annual, "realised"),
            // Long-run USD/PLN volatility is roughly 8-12%; used only before history is synced.
            None => (0.10, "fallback"),
        },
    };
    let drift = match req.drift_pct {
        Some(d) if d.is_finite() && d.abs() < 100.0 => d / 100.0,
        Some(_) => return Err(bad("drift_pct must be in (-100, 100)")),
        None => 0.0,
    };
    let paths = req.paths.unwrap_or(5000);
    if !(100..=MAX_PATHS).contains(&paths) {
        return Err(bad(format!("paths must be in [100, {MAX_PATHS}]")));
    }
    let subjective_prob = req.subjective_prob.map(|p| prob("subjective_prob", p)).transpose()?;

    let pln = terms(req.pln.as_ref(), req.pln_account_id, accounts, Currency::Pln, "pln")?;
    let usd = terms(req.usd.as_ref(), req.usd_account_id, accounts, Currency::Usd, "usd")?;
    let eur = terms(req.eur.as_ref(), req.eur_account_id, accounts, Currency::Eur, "eur")?;

    let input = AnalysisInput {
        start,
        amount_usd: positive("amount_usd", req.amount_usd)?,
        spot,
        usd_eur,
        target: positive("target", req.target)?,
        horizon_days: req.horizon_days as usize,
        spread_pct: pct("spread_pct", req.spread_pct)?,
        pln,
        usd,
        eur,
        gbm: Gbm { drift, vol },
        subjective_prob,
        paths,
        seed: req.seed.unwrap_or(42),
    };
    let resolved = ResolvedInputs {
        start_date: start,
        deadline: start + chrono::Days::new(req.horizon_days as u64),
        spot,
        usd_eur,
        fixing_date,
        vol_pct: vol * 100.0,
        vol_source,
        drift_pct: drift * 100.0,
        pln_rate_pct: input.pln.schedule.rate_on(start),
        usd_rate_pct: input.usd.schedule.rate_on(start),
        eur_rate_pct: input.eur.schedule.rate_on(start),
    };
    Ok((input, resolved))
}

fn terms(
    over: Option<&TermsOverride>,
    account_id: Option<i64>,
    accounts: &[Account],
    ccy: Currency,
    label: &str,
) -> AppResult<SavingsTerms> {
    if let Some(o) = over {
        return Ok(SavingsTerms::constant(
            pct(&format!("{label}.annual_rate_pct"), o.annual_rate_pct)?,
            o.compounding,
            pct(&format!("{label}.tax_pct"), o.tax_pct)?,
        ));
    }
    let id = account_id.ok_or_else(|| bad(format!("{label}_account_id or {label} terms required")))?;
    let acc = accounts.iter().find(|a| a.id == id).ok_or(AppError::NotFound("account"))?;
    if acc.currency != ccy {
        return Err(bad(format!("{label}_account_id must be a {} account", ccy.as_str())));
    }
    Ok(acc.terms())
}
