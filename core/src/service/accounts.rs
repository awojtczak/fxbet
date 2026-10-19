use chrono::NaiveDate;
use serde::Deserialize;

use super::{non_empty, pct};
use crate::domain::fx::Currency;
use crate::domain::savings::Compounding;
use crate::error::{Result, bad};
use crate::model::NewAccount;

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
    pub fn validate(&self) -> Result<NewAccount> {
        Ok(NewAccount {
            name: non_empty("name", &self.name)?,
            currency: self.currency,
            compounding: self.compounding,
            tax_pct: pct("tax_pct", self.tax_pct)?,
        })
    }

    /// Validates a create request: the account plus its first rate.
    pub fn validate_new(&self) -> Result<(NewAccount, f64)> {
        let rate = pct("annual_rate_pct", self.annual_rate_pct.ok_or_else(|| bad("annual_rate_pct is required"))?)?;
        Ok((self.validate()?, rate))
    }
}

#[derive(Deserialize)]
pub struct RateBody {
    pub effective_from: NaiveDate,
    annual_rate_pct: f64,
}

impl RateBody {
    pub fn rate(&self) -> Result<f64> {
        pct("annual_rate_pct", self.annual_rate_pct)
    }
}
