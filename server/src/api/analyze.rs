use axum::Json;
use axum::extract::State;
use fxbet_core::service::analyze::{self, AnalyzeRequest, AnalyzeResponse};

use super::{AppState, blocking};
use crate::db;
use crate::error::AppResult;

pub async fn analyze(State(st): State<AppState>, Json(req): Json<AnalyzeRequest>) -> AppResult<Json<AnalyzeResponse>> {
    let series = db::load_series(&st.pool).await?;
    let accounts = db::list_accounts(&st.pool).await?;
    let today = st.clock.today();
    Ok(Json(blocking(move || analyze::analyze(&req, &series, &accounts, today)).await?))
}
