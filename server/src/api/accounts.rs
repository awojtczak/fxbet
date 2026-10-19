use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use fxbet_core::service::accounts::{AccountBody, RateBody};

use super::AppState;
use crate::db::{self, Account};
use crate::error::AppResult;

pub async fn list(State(st): State<AppState>) -> AppResult<Json<Vec<Account>>> {
    Ok(Json(db::list_accounts(&st.pool).await?))
}

pub async fn create(State(st): State<AppState>, Json(body): Json<AccountBody>) -> AppResult<(StatusCode, Json<Account>)> {
    let (new, rate) = body.validate_new()?;
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

pub async fn add_rate(State(st): State<AppState>, Path(id): Path<i64>, Json(body): Json<RateBody>) -> AppResult<Json<Account>> {
    db::get_account(&st.pool, id).await?;
    db::upsert_account_rate(&st.pool, id, body.effective_from, body.rate()?).await?;
    Ok(Json(db::get_account(&st.pool, id).await?))
}

pub async fn remove_rate(State(st): State<AppState>, Path((id, rate_id)): Path<(i64, i64)>) -> AppResult<Json<Account>> {
    db::delete_account_rate(&st.pool, id, rate_id).await?;
    Ok(Json(db::get_account(&st.pool, id).await?))
}
