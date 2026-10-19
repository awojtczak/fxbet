//! In-browser stand-in for the server's /api router. The web app hands every request to
//! [`LocalApi::handle`] and persists [`LocalApi::store`] / [`LocalApi::fx`] after writes.

mod store;

use std::collections::BTreeMap;

use chrono::NaiveDate;
use fxbet_core::domain::fx::{Fixing, RateSeries};
use fxbet_core::frankfurter;
use fxbet_core::service::accounts::{AccountBody, RateBody};
use fxbet_core::service::analyze::{self, AnalyzeRequest};
use fxbet_core::service::bets::{self, CreateBet, Ctx, NewConversion, UpdateBet};
use fxbet_core::service::fx::{self, HistoryQuery, StatsQuery};
use fxbet_core::{Error, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use crate::store::Store;

#[wasm_bindgen]
pub struct LocalApi {
    store: Store,
    fx: BTreeMap<NaiveDate, Fixing>,
}

/// Fixings as persisted: `[[date, usd_pln, usd_eur], ...]`.
type FxRows = Vec<(NaiveDate, f64, f64)>;

#[wasm_bindgen]
impl LocalApi {
    /// `store` and `fx` are earlier outputs of [`store`](Self::store) and [`fx`](Self::fx), or absent
    /// on first use, in which case default accounts are created as of `today` (YYYY-MM-DD).
    #[wasm_bindgen(constructor)]
    pub fn new(store: Option<String>, fx: Option<String>, today: &str) -> std::result::Result<LocalApi, JsError> {
        let today = parse_date(today)?;
        let store = match store {
            Some(s) => serde_json::from_str(&s).map_err(|e| JsError::new(&format!("stored data unreadable: {e}")))?,
            None => Store::seeded(today),
        };
        let rows: FxRows = match fx {
            Some(s) => serde_json::from_str(&s).map_err(|e| JsError::new(&format!("stored FX rates unreadable: {e}")))?,
            None => Vec::new(),
        };
        let fx = rows.into_iter().map(|(d, usd_pln, usd_eur)| (d, Fixing { usd_pln, usd_eur })).collect();
        Ok(LocalApi { store, fx })
    }

    /// Answers one API call. `url` is the path with an optional query string, `now` an ISO-8601
    /// UTC timestamp. Returns `{"status": u16, "body": json | null}`.
    pub fn handle(&mut self, method: &str, url: &str, body: Option<String>, now: &str) -> String {
        let reply = match self.route(method, url, body.as_deref().unwrap_or("null"), now) {
            Ok((status, body)) => json!({ "status": status, "body": body }),
            Err(e) => json!({ "status": e.status(), "body": { "error": e.to_string() } }),
        };
        reply.to_string()
    }

    pub fn store(&self) -> String {
        serde_json::to_string(&self.store).expect("store serializes")
    }

    pub fn fx(&self) -> String {
        let rows: FxRows = self.fx.iter().map(|(d, f)| (*d, f.usd_pln, f.usd_eur)).collect();
        serde_json::to_string(&rows).expect("fx serializes")
    }

    /// Frankfurter URLs still to fetch to bring the rates up to `today`.
    pub fn sync_urls(&self, base_url: &str, history_start: &str, today: &str) -> std::result::Result<Vec<String>, JsError> {
        let latest = self.fx.keys().next_back().copied();
        let ranges = frankfurter::missing_ranges(latest, parse_date(history_start)?, parse_date(today)?);
        Ok(ranges.into_iter().map(|(from, to)| frankfurter::range_url(base_url, from, to)).collect())
    }

    /// Stores one Frankfurter time-series reply; returns the number of fixings written.
    pub fn add_fixings(&mut self, body: &str) -> std::result::Result<usize, JsError> {
        let fixings = frankfurter::parse(body).map_err(|e| JsError::new(&format!("bad FX reply: {e}")))?;
        let n = fixings.len();
        self.fx.extend(fixings);
        Ok(n)
    }

    pub fn latest_fixing(&self) -> Option<String> {
        self.fx.keys().next_back().map(|d| d.to_string())
    }
}

fn parse_date(s: &str) -> std::result::Result<NaiveDate, JsError> {
    s.get(..10).and_then(|d| d.parse().ok()).ok_or_else(|| JsError::new(&format!("bad date {s:?}")))
}

impl LocalApi {
    fn series(&self) -> RateSeries {
        RateSeries::new(self.fx.clone())
    }

    fn ctx(&self, today: NaiveDate) -> Ctx {
        Ctx::new(self.series(), self.store.accounts(), today)
    }

    fn route(&mut self, method: &str, url: &str, body: &str, now: &str) -> Result<(u16, Value)> {
        let today = now.get(..10).and_then(|d| d.parse::<NaiveDate>().ok()).ok_or_else(|| Error::Internal(format!("bad clock {now:?}")))?;
        let (path, query) = url.split_once('?').unwrap_or((url, ""));
        let segs: Vec<&str> = path.trim_matches('/').split('/').collect();
        let id = |s: &str| s.parse::<i64>().map_err(|_| Error::NotFound("route"));
        match (method, segs.as_slice()) {
            ("GET", ["api", "health"]) => ok("ok"),
            ("GET", ["api", "fx", "latest"]) => ok(fx::latest(&self.series())?),
            ("GET", ["api", "fx", "history"]) => ok(fx::history(&self.series(), &parse_query::<HistoryQuery>(query)?, today)?),
            ("GET", ["api", "fx", "stats"]) => ok(fx::stats(&self.series(), &parse_query::<StatsQuery>(query)?)?),

            ("GET", ["api", "accounts"]) => ok(self.store.accounts()),
            ("POST", ["api", "accounts"]) => {
                let (new, rate) = parse_body::<AccountBody>(body)?.validate_new()?;
                let id = self.store.insert_account(&new, now);
                self.store.upsert_rate(id, today, rate);
                created(self.store.account(id)?)
            }
            ("PUT", ["api", "accounts", a]) => {
                let a = id(a)?;
                self.store.update_account(a, &parse_body::<AccountBody>(body)?.validate()?)?;
                ok(self.store.account(a)?)
            }
            ("DELETE", ["api", "accounts", a]) => {
                self.store.delete_account(id(a)?)?;
                no_content()
            }
            ("POST", ["api", "accounts", a, "rates"]) => {
                let a = id(a)?;
                self.store.account(a)?;
                let b = parse_body::<RateBody>(body)?;
                self.store.upsert_rate(a, b.effective_from, b.rate()?);
                ok(self.store.account(a)?)
            }
            ("DELETE", ["api", "accounts", a, "rates", r]) => {
                let a = id(a)?;
                self.store.delete_rate(a, id(r)?)?;
                ok(self.store.account(a)?)
            }

            ("POST", ["api", "analyze"]) => {
                let req = parse_body::<AnalyzeRequest>(body)?;
                ok(analyze::analyze(&req, &self.series(), &self.store.accounts(), today)?)
            }

            ("GET", ["api", "bets"]) => {
                let ctx = self.ctx(today);
                ok(ctx.list(self.store.bets(), self.store.conversions_by_bet())?)
            }
            ("POST", ["api", "bets"]) => {
                let (bet, conversions) = bets::prepare(parse_body::<CreateBet>(body)?, &self.series(), &self.store.accounts(), today)?;
                let id = self.store.insert_bet(bet, &conversions, now);
                created(self.detail(id, today)?)
            }
            ("GET", ["api", "bets", b]) => ok(self.detail(id(b)?, today)?),
            ("PATCH", ["api", "bets", b]) => {
                let b = id(b)?;
                let u = parse_body::<UpdateBet>(body)?;
                self.store.update_bet_notes(b, u.title()?, u.notes)?;
                ok(self.detail(b, today)?)
            }
            ("DELETE", ["api", "bets", b]) => {
                self.store.delete_bet(id(b)?)?;
                no_content()
            }
            ("POST", ["api", "bets", b, "conversions"]) => {
                let b = id(b)?;
                let bet = self.store.bet(b)?;
                let c = parse_body::<NewConversion>(body)?.validate(bet.start_date, today)?;
                self.store.insert_conversion(b, &c);
                created(self.detail(b, today)?)
            }
            ("DELETE", ["api", "bets", b, "conversions", c]) => {
                let b = id(b)?;
                self.store.delete_conversion(b, id(c)?)?;
                ok(self.detail(b, today)?)
            }
            ("GET", ["api", "calibration"]) => {
                let ctx = self.ctx(today);
                ok(ctx.calibration(self.store.bets(), self.store.conversions_by_bet())?)
            }
            _ => Err(Error::NotFound("route")),
        }
    }

    fn detail(&self, id: i64, today: NaiveDate) -> Result<bets::BetDetail> {
        self.ctx(today).detail(self.store.bet(id)?, self.store.conversions(id))
    }
}

fn parse_body<T: DeserializeOwned>(body: &str) -> Result<T> {
    serde_json::from_str(body).map_err(|e| Error::BadRequest(format!("invalid request body: {e}")))
}

fn parse_query<T: DeserializeOwned>(query: &str) -> Result<T> {
    serde_urlencoded::from_str(query).map_err(|e| Error::BadRequest(format!("invalid query: {e}")))
}

fn ok(v: impl Serialize) -> Result<(u16, Value)> {
    Ok((200, serde_json::to_value(v).map_err(|e| Error::Internal(e.to_string()))?))
}

fn created(v: impl Serialize) -> Result<(u16, Value)> {
    Ok((201, ok(v)?.1))
}

fn no_content() -> Result<(u16, Value)> {
    Ok((204, Value::Null))
}

#[cfg(test)]
mod tests;
