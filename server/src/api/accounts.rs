use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::NaiveDate;
use serde::Deserialize;

use super::{AppState, non_empty, pct};
use crate::db::{self, Account, NewAccount};
use crate::domain::fx::Currency;
use crate::domain::savings::Compounding;
use crate::error::{AppResult, bad};

#[derive(Deserialize)]
pub struct AccountBody {
    name: String,
    currency: Currency,
    compounding: Compounding,
    tax_pct: f64,
    /// Required on create: the rate effective from today.
    annual_rate_pct: Option<f64>,
}

impl AccountBody {
    fn validate(&self) -> AppResult<NewAccount> {
        Ok(NewAccount {
            name: non_empty("name", &self.name)?,
            currency: self.currency,
            compounding: self.compounding,
            tax_pct: pct("tax_pct", self.tax_pct)?,
        })
    }
}

pub async fn list(State(st): State<AppState>) -> AppResult<Json<Vec<Account>>> {
    Ok(Json(db::list_accounts(&st.pool).await?))
}

pub async fn create(State(st): State<AppState>, Json(body): Json<AccountBody>) -> AppResult<(StatusCode, Json<Account>)> {
    let new = body.validate()?;
    let rate = pct("annual_rate_pct", body.annual_rate_pct.ok_or_else(|| bad("annual_rate_pct is required"))?)?;
    let id = db::insert_account(&st.pool, &new).await?;
    db::upsert_account_rate(&st.pool, id, st.clock.today(), rate).await?;
    tracing::info!(account = id, "account created");
    Ok((StatusCode::CREATED, Json(db::get_account(&st.pool, id).await?)))
}

pub async fn update(State(st): State<AppState>, Path(id): Path<i64>, Json(body): Json<AccountBody>) -> AppResult<Json<Account>> {
    db::update_account(&st.pool, id, &body.validate()?).await?;
    Ok(Json(db::get_account(&st.pool, id).await?))
}

pub async fn remove(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<StatusCode> {
    db::delete_account(&st.pool, id).await?;
    tracing::info!(account = id, "account deleted");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct RateBody {
    effective_from: NaiveDate,
    annual_rate_pct: f64,
}

pub async fn add_rate(State(st): State<AppState>, Path(id): Path<i64>, Json(body): Json<RateBody>) -> AppResult<Json<Account>> {
    db::get_account(&st.pool, id).await?;
    db::upsert_account_rate(&st.pool, id, body.effective_from, pct("annual_rate_pct", body.annual_rate_pct)?).await?;
    Ok(Json(db::get_account(&st.pool, id).await?))
}

pub async fn remove_rate(State(st): State<AppState>, Path((id, rate_id)): Path<(i64, i64)>) -> AppResult<Json<Account>> {
    db::delete_account_rate(&st.pool, id, rate_id).await?;
    Ok(Json(db::get_account(&st.pool, id).await?))
}
