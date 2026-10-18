//! SQLite persistence. Rows are mapped to domain types here; nothing above this module
//! sees SQL types.

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::domain::fx::{Currency, Fixing, RateSeries};
use crate::domain::savings::{Compounding, RateSchedule, SavingsTerms};
use crate::error::{AppError, AppResult};

pub async fn connect(url: &str) -> Result<SqlitePool> {
    if let Some(path) = url.strip_prefix("sqlite://").and_then(|p| p.split('?').next())
        && let Some(dir) = std::path::Path::new(path).parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).with_context(|| format!("creating database dir {}", dir.display()))?;
    }
    let opts = SqliteConnectOptions::from_str(url)?.create_if_missing(true).foreign_keys(true);
    // Each in-memory connection is its own database, so tests must share a single one.
    let max = if url.contains(":memory:") { 1 } else { 5 };
    let pool = SqlitePoolOptions::new().max_connections(max).connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Creates placeholder accounts on an empty database so the analyzer works out of the box.
pub async fn seed_default_accounts(pool: &SqlitePool, today: NaiveDate) -> Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts").fetch_one(pool).await?;
    if count > 0 {
        return Ok(());
    }
    let defaults = [
        ("PLN savings (daily capitalization)", Currency::Pln, Compounding::Daily, 5.0),
        ("USD savings (variable)", Currency::Usd, Compounding::Monthly, 3.0),
        ("EUR savings (variable)", Currency::Eur, Compounding::Monthly, 2.0),
    ];
    for (name, ccy, comp, rate) in defaults {
        let id = insert_account(pool, &NewAccount { name: name.into(), currency: ccy, compounding: comp, tax_pct: 19.0 }).await?;
        upsert_account_rate(pool, id, today, rate).await?;
    }
    tracing::info!(accounts = 3, "seeded default accounts");
    Ok(())
}

// ---------- FX ----------

pub async fn upsert_fixings(pool: &SqlitePool, fixings: &[(NaiveDate, Fixing)], source: &str) -> Result<usize> {
    let mut tx = pool.begin().await?;
    for (date, f) in fixings {
        sqlx::query(
            "INSERT INTO fx_rates (date, usd_pln, usd_eur, source) VALUES (?, ?, ?, ?)
             ON CONFLICT(date) DO UPDATE SET usd_pln = excluded.usd_pln, usd_eur = excluded.usd_eur, source = excluded.source",
        )
        .bind(date)
        .bind(f.usd_pln)
        .bind(f.usd_eur)
        .bind(source)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(fixings.len())
}

pub async fn load_series(pool: &SqlitePool) -> Result<RateSeries> {
    let rows: Vec<(NaiveDate, f64, f64)> =
        sqlx::query_as("SELECT date, usd_pln, usd_eur FROM fx_rates ORDER BY date").fetch_all(pool).await?;
    let map: BTreeMap<NaiveDate, Fixing> =
        rows.into_iter().map(|(d, usd_pln, usd_eur)| (d, Fixing { usd_pln, usd_eur })).collect();
    Ok(RateSeries::new(map))
}

pub async fn latest_fixing_date(pool: &SqlitePool) -> Result<Option<NaiveDate>> {
    Ok(sqlx::query_scalar("SELECT MAX(date) FROM fx_rates").fetch_one(pool).await?)
}

// ---------- Accounts ----------

#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub currency: Currency,
    pub compounding: Compounding,
    pub tax_pct: f64,
    pub current_rate_pct: f64,
    pub rates: Vec<AccountRate>,
}

impl Account {
    pub fn terms(&self) -> SavingsTerms {
        let steps = self.rates.iter().map(|r| (r.effective_from, r.annual_rate_pct)).collect();
        SavingsTerms {
            schedule: RateSchedule::from_steps(steps).unwrap_or_else(|| RateSchedule::constant(0.0)),
            compounding: self.compounding,
            tax_pct: self.tax_pct,
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AccountRate {
    pub id: i64,
    pub account_id: i64,
    pub effective_from: NaiveDate,
    pub annual_rate_pct: f64,
}

#[derive(Debug, Clone)]
pub struct NewAccount {
    pub name: String,
    pub currency: Currency,
    pub compounding: Compounding,
    pub tax_pct: f64,
}

#[derive(FromRow)]
struct AccountRow {
    id: i64,
    name: String,
    currency: String,
    compounding: String,
    tax_pct: f64,
}

fn to_account(row: AccountRow, mut rates: Vec<AccountRate>) -> Result<Account> {
    rates.sort_by_key(|r| r.effective_from);
    Ok(Account {
        id: row.id,
        currency: Currency::parse(&row.currency).with_context(|| format!("account={} bad currency", row.id))?,
        compounding: Compounding::parse(&row.compounding).with_context(|| format!("account={} bad compounding", row.id))?,
        current_rate_pct: rates.last().map(|r| r.annual_rate_pct).unwrap_or(0.0),
        name: row.name,
        tax_pct: row.tax_pct,
        rates,
    })
}

pub async fn list_accounts(pool: &SqlitePool) -> Result<Vec<Account>> {
    let rows: Vec<AccountRow> =
        sqlx::query_as("SELECT id, name, currency, compounding, tax_pct FROM accounts ORDER BY id").fetch_all(pool).await?;
    let rates: Vec<AccountRate> =
        sqlx::query_as("SELECT id, account_id, effective_from, annual_rate_pct FROM account_rates").fetch_all(pool).await?;
    let mut by_account: HashMap<i64, Vec<AccountRate>> = HashMap::new();
    for r in rates {
        by_account.entry(r.account_id).or_default().push(r);
    }
    rows.into_iter().map(|row| {
        let rates = by_account.remove(&row.id).unwrap_or_default();
        to_account(row, rates)
    }).collect()
}

pub async fn get_account(pool: &SqlitePool, id: i64) -> AppResult<Account> {
    let row: AccountRow = sqlx::query_as("SELECT id, name, currency, compounding, tax_pct FROM accounts WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound("account"))?;
    let rates: Vec<AccountRate> =
        sqlx::query_as("SELECT id, account_id, effective_from, annual_rate_pct FROM account_rates WHERE account_id = ?")
            .bind(id)
            .fetch_all(pool)
            .await?;
    Ok(to_account(row, rates)?)
}

pub async fn insert_account(pool: &SqlitePool, a: &NewAccount) -> Result<i64> {
    let id = sqlx::query("INSERT INTO accounts (name, currency, compounding, tax_pct, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&a.name)
        .bind(a.currency.as_str())
        .bind(a.compounding.as_str())
        .bind(a.tax_pct)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await?
        .last_insert_rowid();
    Ok(id)
}

pub async fn update_account(pool: &SqlitePool, id: i64, a: &NewAccount) -> AppResult<()> {
    let n = sqlx::query("UPDATE accounts SET name = ?, currency = ?, compounding = ?, tax_pct = ? WHERE id = ?")
        .bind(&a.name)
        .bind(a.currency.as_str())
        .bind(a.compounding.as_str())
        .bind(a.tax_pct)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if n == 0 { Err(AppError::NotFound("account")) } else { Ok(()) }
}

pub async fn delete_account(pool: &SqlitePool, id: i64) -> AppResult<()> {
    // Checked explicitly: SQLite reports RESTRICT violations with a generic constraint code.
    let used: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bets WHERE pln_account_id = ?1 OR usd_account_id = ?1 OR eur_account_id = ?1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    if used > 0 {
        return Err(AppError::Conflict(format!("account={id} is used by {used} bet(s); delete those bets first")));
    }
    let n = sqlx::query("DELETE FROM accounts WHERE id = ?").bind(id).execute(pool).await?.rows_affected();
    if n == 0 { Err(AppError::NotFound("account")) } else { Ok(()) }
}

pub async fn upsert_account_rate(pool: &SqlitePool, account_id: i64, from: NaiveDate, rate_pct: f64) -> Result<()> {
    sqlx::query(
        "INSERT INTO account_rates (account_id, effective_from, annual_rate_pct) VALUES (?, ?, ?)
         ON CONFLICT(account_id, effective_from) DO UPDATE SET annual_rate_pct = excluded.annual_rate_pct",
    )
    .bind(account_id)
    .bind(from)
    .bind(rate_pct)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_account_rate(pool: &SqlitePool, account_id: i64, rate_id: i64) -> AppResult<()> {
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM account_rates WHERE account_id = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await?;
    if remaining <= 1 {
        return Err(AppError::Conflict("an account needs at least one rate".into()));
    }
    let n = sqlx::query("DELETE FROM account_rates WHERE id = ? AND account_id = ?")
        .bind(rate_id)
        .bind(account_id)
        .execute(pool)
        .await?
        .rows_affected();
    if n == 0 { Err(AppError::NotFound("rate")) } else { Ok(()) }
}

// ---------- Bets ----------

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Bet {
    pub id: i64,
    pub created_at: String,
    pub title: String,
    pub notes: String,
    pub amount_usd: f64,
    pub start_date: NaiveDate,
    pub deadline: NaiveDate,
    pub spot_usd_pln: f64,
    pub spot_usd_eur: f64,
    pub target_rate: f64,
    pub spread_pct: f64,
    pub subjective_prob: Option<f64>,
    pub model_prob: f64,
    pub pln_account_id: i64,
    pub usd_account_id: i64,
    pub eur_account_id: i64,
    #[sqlx(json)]
    pub snapshot: serde_json::Value,
    pub retrospective: bool,
}

const BET_COLUMNS: &str = "id, created_at, title, notes, amount_usd, start_date, deadline, spot_usd_pln, spot_usd_eur, \
    target_rate, spread_pct, subjective_prob, model_prob, pln_account_id, usd_account_id, eur_account_id, snapshot, retrospective";

#[derive(Debug, Clone)]
pub struct NewBet {
    pub title: String,
    pub notes: String,
    pub amount_usd: f64,
    pub start_date: NaiveDate,
    pub deadline: NaiveDate,
    pub spot_usd_pln: f64,
    pub spot_usd_eur: f64,
    pub target_rate: f64,
    pub spread_pct: f64,
    pub subjective_prob: Option<f64>,
    pub model_prob: f64,
    pub pln_account_id: i64,
    pub usd_account_id: i64,
    pub eur_account_id: i64,
    pub snapshot: serde_json::Value,
    pub retrospective: bool,
}

/// Takes a connection so callers can insert a bet and its conversions in one transaction.
pub async fn insert_bet(conn: &mut SqliteConnection, b: &NewBet) -> Result<i64> {
    let id = sqlx::query(
        "INSERT INTO bets (created_at, title, notes, amount_usd, start_date, deadline, spot_usd_pln, spot_usd_eur, target_rate,
            spread_pct, subjective_prob, model_prob, pln_account_id, usd_account_id, eur_account_id, snapshot, retrospective)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(&b.title)
    .bind(&b.notes)
    .bind(b.amount_usd)
    .bind(b.start_date)
    .bind(b.deadline)
    .bind(b.spot_usd_pln)
    .bind(b.spot_usd_eur)
    .bind(b.target_rate)
    .bind(b.spread_pct)
    .bind(b.subjective_prob)
    .bind(b.model_prob)
    .bind(b.pln_account_id)
    .bind(b.usd_account_id)
    .bind(b.eur_account_id)
    .bind(b.snapshot.to_string())
    .bind(b.retrospective)
    .execute(&mut *conn)
    .await?
    .last_insert_rowid();
    Ok(id)
}

pub async fn list_bets(pool: &SqlitePool) -> Result<Vec<Bet>> {
    Ok(sqlx::query_as(&format!("SELECT {BET_COLUMNS} FROM bets ORDER BY start_date DESC, id DESC")).fetch_all(pool).await?)
}

pub async fn get_bet(pool: &SqlitePool, id: i64) -> AppResult<Bet> {
    sqlx::query_as(&format!("SELECT {BET_COLUMNS} FROM bets WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound("bet"))
}

pub async fn update_bet_notes(pool: &SqlitePool, id: i64, title: &str, notes: &str) -> AppResult<()> {
    let n = sqlx::query("UPDATE bets SET title = ?, notes = ? WHERE id = ?")
        .bind(title)
        .bind(notes)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if n == 0 { Err(AppError::NotFound("bet")) } else { Ok(()) }
}

pub async fn delete_bet(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let n = sqlx::query("DELETE FROM bets WHERE id = ?").bind(id).execute(pool).await?.rows_affected();
    if n == 0 { Err(AppError::NotFound("bet")) } else { Ok(()) }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ConversionRecord {
    pub id: i64,
    pub bet_id: i64,
    pub date: NaiveDate,
    pub usd_amount: f64,
    pub rate: f64,
    pub note: String,
}

pub async fn list_conversions(pool: &SqlitePool, bet_id: i64) -> Result<Vec<ConversionRecord>> {
    Ok(sqlx::query_as("SELECT id, bet_id, date, usd_amount, rate, note FROM conversions WHERE bet_id = ? ORDER BY date, id")
        .bind(bet_id)
        .fetch_all(pool)
        .await?)
}

pub async fn list_all_conversions(pool: &SqlitePool) -> Result<HashMap<i64, Vec<ConversionRecord>>> {
    let rows: Vec<ConversionRecord> =
        sqlx::query_as("SELECT id, bet_id, date, usd_amount, rate, note FROM conversions ORDER BY date, id").fetch_all(pool).await?;
    let mut map: HashMap<i64, Vec<ConversionRecord>> = HashMap::new();
    for r in rows {
        map.entry(r.bet_id).or_default().push(r);
    }
    Ok(map)
}

pub async fn insert_conversion(conn: &mut SqliteConnection, bet_id: i64, date: NaiveDate, usd_amount: f64, rate: f64, note: &str) -> Result<i64> {
    Ok(sqlx::query("INSERT INTO conversions (bet_id, date, usd_amount, rate, note) VALUES (?, ?, ?, ?, ?)")
        .bind(bet_id)
        .bind(date)
        .bind(usd_amount)
        .bind(rate)
        .bind(note)
        .execute(&mut *conn)
        .await?
        .last_insert_rowid())
}

pub async fn delete_conversion(pool: &SqlitePool, bet_id: i64, id: i64) -> AppResult<()> {
    let n = sqlx::query("DELETE FROM conversions WHERE id = ? AND bet_id = ?")
        .bind(id)
        .bind(bet_id)
        .execute(pool)
        .await?
        .rows_affected();
    if n == 0 { Err(AppError::NotFound("conversion")) } else { Ok(()) }
}
