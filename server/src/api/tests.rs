//! End-to-end through the HTTP router with an in-memory database and a fixed clock.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Days, NaiveDate};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::{AppState, router};
use crate::clock::Clock;
use crate::config::Config;
use crate::db;
use crate::domain::fx::Fixing;
use crate::fxsync::FxProvider;

struct FixedClock(NaiveDate);

impl Clock for FixedClock {
    fn today(&self) -> NaiveDate {
        self.0
    }
}

fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

async fn app() -> Router {
    let today = d("2026-10-16");
    let pool = db::connect("sqlite::memory:").await.unwrap();
    db::seed_default_accounts(&pool, today).await.unwrap();
    // 120 days of a gently rising USD/PLN, 3.80 -> ~3.92.
    let fixings: Vec<(NaiveDate, Fixing)> = (0..120u64)
        .map(|i| {
            let date = today - Days::new(119 - i);
            (date, Fixing { usd_pln: 3.80 + 0.001 * i as f64 + 0.004 * ((i as f64) * 0.7).sin(), usd_eur: 0.89 })
        })
        .collect();
    db::upsert_fixings(&pool, &fixings, "test").await.unwrap();
    let mut config = Config::from_env().unwrap();
    config.fx_provider_url = "http://127.0.0.1:9".into();
    router(AppState {
        pool,
        clock: Arc::new(FixedClock(today)),
        provider: Arc::new(FxProvider::new(config.fx_provider_url.clone()).unwrap()),
        config: Arc::new(config),
    })
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let req = Request::builder().method(method).uri(uri).header("content-type", "application/json");
    let req = req.body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty)).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn analysis_body() -> Value {
    json!({
        "amount_usd": 10000, "target": 3.93, "horizon_days": 90,
        "pln_account_id": 1, "usd_account_id": 2, "eur_account_id": 3,
        "subjective_prob": 0.6, "paths": 1000
    })
}

#[tokio::test]
async fn analyze_uses_latest_fixing_and_realised_vol() {
    let app = app().await;
    let (status, body) = call(&app, "POST", "/api/analyze", Some(analysis_body())).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["inputs"]["fixing_date"], "2026-10-16");
    assert_eq!(body["inputs"]["vol_source"], "realised");
    let gain = body["analysis"]["headline"]["fx_gain_pln"].as_f64().unwrap();
    let spot = body["inputs"]["spot"].as_f64().unwrap();
    assert!((gain - 10000.0 * (3.93 - spot)).abs() < 1e-6);
}

#[tokio::test]
async fn analyze_rejects_bad_input() {
    let app = app().await;
    let mut body = analysis_body();
    body["horizon_days"] = json!(0);
    let (status, err) = call(&app, "POST", "/api/analyze", Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["error"].as_str().unwrap().contains("horizon_days"));
}

#[tokio::test]
async fn bet_lifecycle_tracks_conversion_and_resolves() {
    let app = app().await;
    let mut analysis = analysis_body();
    analysis["start_date"] = json!("2026-07-01");
    analysis["horizon_days"] = json!(60);
    analysis["target"] = json!(3.86);
    let (status, created) =
        call(&app, "POST", "/api/bets", Some(json!({ "title": "backdated", "analysis": analysis }))).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["bet"]["id"].as_i64().unwrap();
    assert_eq!(created["status"], "hit");

    let (status, after) = call(
        &app,
        "POST",
        &format!("/api/bets/{id}/conversions"),
        Some(json!({ "date": "2026-08-20", "usd_amount": 10000, "rate": 3.87 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{after}");
    assert_eq!(after["tracking"]["summary"]["converted_usd"].as_f64().unwrap(), 10000.0);

    let (status, cal) = call(&app, "GET", "/api/calibration", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cal["calibration"]["resolved"], 1);

    let (status, _) = call(&app, "DELETE", "/api/accounts/1", None).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn conversion_outside_bet_window_is_rejected() {
    let app = app().await;
    let (_, created) = call(&app, "POST", "/api/bets", Some(json!({ "title": "t", "analysis": analysis_body() }))).await;
    let id = created["bet"]["id"].as_i64().unwrap();
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/bets/{id}/conversions"),
        Some(json!({ "date": "2026-01-01", "usd_amount": 1, "rate": 3.9 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
