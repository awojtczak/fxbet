//! Same scenarios as the server's HTTP tests, through the in-browser router.

use chrono::{Days, NaiveDate};
use serde_json::{Value, json};

use super::LocalApi;

const NOW: &str = "2026-10-16T12:00:00.000Z";

fn api() -> LocalApi {
    let today: NaiveDate = "2026-10-16".parse().unwrap();
    // 120 days of a gently rising USD/PLN, 3.80 -> ~3.92 (as in the server tests).
    let rows: Vec<Value> = (0..120u64)
        .map(|i| {
            let date = today - Days::new(119 - i);
            json!([date, 3.80 + 0.001 * i as f64 + 0.004 * ((i as f64) * 0.7).sin(), 0.89])
        })
        .collect();
    LocalApi::new(None, Some(Value::Array(rows).to_string()), "2026-10-16").unwrap()
}

fn call(api: &mut LocalApi, method: &str, url: &str, body: Option<Value>) -> (u16, Value) {
    let reply: Value = serde_json::from_str(&api.handle(method, url, body.map(|b| b.to_string()), NOW)).unwrap();
    (reply["status"].as_u64().unwrap() as u16, reply["body"].clone())
}

fn analysis_body() -> Value {
    json!({
        "amount_usd": 10000, "target": 3.93, "horizon_days": 90,
        "pln_account_id": 1, "usd_account_id": 2, "eur_account_id": 3,
        "subjective_prob": 0.6, "paths": 1000
    })
}

#[test]
fn seeds_default_accounts_and_serves_rates() {
    let mut api = api();
    let (status, accounts) = call(&mut api, "GET", "/api/accounts", None);
    assert_eq!(status, 200);
    assert_eq!(accounts.as_array().unwrap().len(), 3);
    let (_, latest) = call(&mut api, "GET", "/api/fx/latest", None);
    assert_eq!(latest["date"], "2026-10-16");
    let (status, history) = call(&mut api, "GET", "/api/fx/history?from=2026-10-01", None);
    assert_eq!(status, 200);
    assert_eq!(history.as_array().unwrap().len(), 16);
}

#[test]
fn analyze_uses_latest_fixing_and_rejects_bad_input() {
    let mut api = api();
    let (status, body) = call(&mut api, "POST", "/api/analyze", Some(analysis_body()));
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["inputs"]["vol_source"], "realised");
    let mut bad = analysis_body();
    bad["horizon_days"] = json!(0);
    let (status, err) = call(&mut api, "POST", "/api/analyze", Some(bad));
    assert_eq!(status, 400);
    assert!(err["error"].as_str().unwrap().contains("horizon_days"));
}

#[test]
fn bet_lifecycle_survives_a_reload() {
    let mut api = api();
    let mut analysis = analysis_body();
    analysis["start_date"] = json!("2026-07-01");
    analysis["horizon_days"] = json!(60);
    analysis["target"] = json!(3.86);
    let (status, created) = call(&mut api, "POST", "/api/bets", Some(json!({ "title": "backdated", "analysis": analysis })));
    assert_eq!(status, 201, "{created}");
    let id = created["bet"]["id"].as_i64().unwrap();
    assert_eq!(created["status"], "hit");

    let (status, after) = call(
        &mut api,
        "POST",
        &format!("/api/bets/{id}/conversions"),
        Some(json!({ "date": "2026-08-20", "usd_amount": 10000, "rate": 3.87 })),
    );
    assert_eq!(status, 201, "{after}");

    let mut api = LocalApi::new(Some(api.store()), Some(api.fx()), "2026-10-16").unwrap();
    let (_, detail) = call(&mut api, "GET", &format!("/api/bets/{id}"), None);
    assert_eq!(detail["tracking"]["summary"]["converted_usd"].as_f64().unwrap(), 10000.0);
    let (_, cal) = call(&mut api, "GET", "/api/calibration", None);
    assert_eq!(cal["calibration"]["resolved"], 1);

    let (status, _) = call(&mut api, "DELETE", "/api/accounts/1", None);
    assert_eq!(status, 409);
    let (status, _) = call(&mut api, "DELETE", &format!("/api/bets/{id}"), None);
    assert_eq!(status, 204);
    let (status, _) = call(&mut api, "DELETE", "/api/accounts/1", None);
    assert_eq!(status, 204);
}

#[test]
fn account_keeps_at_least_one_rate() {
    let mut api = api();
    let (_, acc) = call(&mut api, "GET", "/api/accounts", None);
    let rate_id = acc[0]["rates"][0]["id"].as_i64().unwrap();
    let (status, _) = call(&mut api, "DELETE", &format!("/api/accounts/1/rates/{rate_id}"), None);
    assert_eq!(status, 409);
    let (status, acc) = call(&mut api, "POST", "/api/accounts/1/rates", Some(json!({ "effective_from": "2026-11-01", "annual_rate_pct": 4.5 })));
    assert_eq!(status, 200);
    assert_eq!(acc["current_rate_pct"], 4.5);
}

#[test]
fn sync_urls_and_fixings() {
    let mut api = LocalApi::new(None, None, "2026-10-16").unwrap();
    assert_eq!(api.sync_urls("https://x/v1", "2026-01-01", "2026-10-16").unwrap(), vec!["https://x/v1/2026-01-01..2026-10-16?base=USD&symbols=PLN,EUR"]);
    assert_eq!(api.add_fixings(r#"{"rates":{"2026-10-16":{"PLN":3.88,"EUR":0.89}}}"#).unwrap(), 1);
    assert_eq!(api.latest_fixing().as_deref(), Some("2026-10-16"));
    let (status, _) = call(&mut api, "GET", "/api/nope", None);
    assert_eq!(status, 404);
}
