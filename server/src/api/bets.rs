use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use fxbet_core::service::bets::{self, BetDetail, BetListItem, CalibrationResponse, Ctx, CreateBet, NewConversion, UpdateBet};

use super::{AppState, blocking};
use crate::db;
use crate::error::AppResult;

pub async fn create(State(st): State<AppState>, Json(body): Json<CreateBet>) -> AppResult<(StatusCode, Json<BetDetail>)> {
    let series = db::load_series(&st.pool).await?;
    let accounts = db::list_accounts(&st.pool).await?;
    let today = st.clock.today();
    let (bet, conversions) = blocking(move || bets::prepare(body, &series, &accounts, today)).await?;
    let mut tx = st.pool.begin().await?;
    let id = db::insert_bet(&mut tx, &bet).await?;
    for c in &conversions {
        db::insert_conversion(&mut tx, id, c.date, c.usd_amount, c.rate, &c.note).await?;
    }
    tx.commit().await?;
    tracing::info!(
        bet = id,
        amount_usd = bet.amount_usd,
        target = bet.target_rate,
        retrospective = bet.retrospective,
        conversions = conversions.len(),
        "bet created"
    );
    Ok((StatusCode::CREATED, Json(load_detail(&st, id).await?)))
}

pub async fn list(State(st): State<AppState>) -> AppResult<Json<Vec<BetListItem>>> {
    let ctx = ctx(&st).await?;
    Ok(Json(ctx.list(db::list_bets(&st.pool).await?, db::list_all_conversions(&st.pool).await?)?))
}

pub async fn detail(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<BetDetail>> {
    Ok(Json(load_detail(&st, id).await?))
}

pub async fn update(State(st): State<AppState>, Path(id): Path<i64>, Json(body): Json<UpdateBet>) -> AppResult<Json<BetDetail>> {
    db::update_bet_notes(&st.pool, id, &body.title()?, &body.notes).await?;
    Ok(Json(load_detail(&st, id).await?))
}

pub async fn remove(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<StatusCode> {
    db::delete_bet(&st.pool, id).await?;
    tracing::info!(bet = id, "bet deleted");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_conversion(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<NewConversion>,
) -> AppResult<(StatusCode, Json<BetDetail>)> {
    let bet = db::get_bet(&st.pool, id).await?;
    let c = body.validate(bet.start_date, st.clock.today())?;
    db::insert_conversion(&mut *st.pool.acquire().await?, id, c.date, c.usd_amount, c.rate, &c.note).await?;
    tracing::info!(bet = id, usd_amount = c.usd_amount, rate = c.rate, date = %c.date, "conversion recorded");
    Ok((StatusCode::CREATED, Json(load_detail(&st, id).await?)))
}

pub async fn remove_conversion(State(st): State<AppState>, Path((id, conversion_id)): Path<(i64, i64)>) -> AppResult<Json<BetDetail>> {
    db::delete_conversion(&st.pool, id, conversion_id).await?;
    Ok(Json(load_detail(&st, id).await?))
}

pub async fn calibration(State(st): State<AppState>) -> AppResult<Json<CalibrationResponse>> {
    let ctx = ctx(&st).await?;
    Ok(Json(ctx.calibration(db::list_bets(&st.pool).await?, db::list_all_conversions(&st.pool).await?)?))
}

async fn ctx(st: &AppState) -> AppResult<Ctx> {
    Ok(Ctx::new(db::load_series(&st.pool).await?, db::list_accounts(&st.pool).await?, st.clock.today()))
}

async fn load_detail(st: &AppState, id: i64) -> AppResult<BetDetail> {
    let bet = db::get_bet(&st.pool, id).await?;
    let conversions = db::list_conversions(&st.pool, id).await?;
    Ok(ctx(st).await?.detail(bet, conversions)?)
}
