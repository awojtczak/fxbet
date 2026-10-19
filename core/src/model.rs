//! Persisted records, as both storage backends hold them.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::fx::Currency;
use crate::domain::savings::{Compounding, RateSchedule, SavingsTerms};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Builds an account from its row and its rate schedule rows in any order.
    pub fn assemble(id: i64, name: String, currency: Currency, compounding: Compounding, tax_pct: f64, mut rates: Vec<AccountRate>) -> Self {
        rates.sort_by_key(|r| r.effective_from);
        Account { id, name, currency, compounding, tax_pct, current_rate_pct: rates.last().map(|r| r.annual_rate_pct).unwrap_or(0.0), rates }
    }

    pub fn terms(&self) -> SavingsTerms {
        let steps = self.rates.iter().map(|r| (r.effective_from, r.annual_rate_pct)).collect();
        SavingsTerms {
            schedule: RateSchedule::from_steps(steps).unwrap_or_else(|| RateSchedule::constant(0.0)),
            compounding: self.compounding,
            tax_pct: self.tax_pct,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
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

/// Placeholder accounts created on an empty store so the analyzer works out of the box.
pub const DEFAULT_ACCOUNTS: [(&str, Currency, Compounding, f64); 3] = [
    ("PLN savings (daily capitalization)", Currency::Pln, Compounding::Daily, 5.0),
    ("USD savings (variable)", Currency::Usd, Compounding::Monthly, 3.0),
    ("EUR savings (variable)", Currency::Eur, Compounding::Monthly, 2.0),
];
pub const DEFAULT_TAX_PCT: f64 = 19.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
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
    #[cfg_attr(feature = "sqlx", sqlx(json))]
    pub snapshot: serde_json::Value,
    pub retrospective: bool,
}

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

impl NewBet {
    pub fn into_bet(self, id: i64, created_at: String) -> Bet {
        Bet {
            id,
            created_at,
            title: self.title,
            notes: self.notes,
            amount_usd: self.amount_usd,
            start_date: self.start_date,
            deadline: self.deadline,
            spot_usd_pln: self.spot_usd_pln,
            spot_usd_eur: self.spot_usd_eur,
            target_rate: self.target_rate,
            spread_pct: self.spread_pct,
            subjective_prob: self.subjective_prob,
            model_prob: self.model_prob,
            pln_account_id: self.pln_account_id,
            usd_account_id: self.usd_account_id,
            eur_account_id: self.eur_account_id,
            snapshot: self.snapshot,
            retrospective: self.retrospective,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct ConversionRecord {
    pub id: i64,
    pub bet_id: i64,
    pub date: NaiveDate,
    pub usd_amount: f64,
    pub rate: f64,
    pub note: String,
}
