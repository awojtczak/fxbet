//! Records held in the browser. Mirrors the SQLite schema's constraints (cascades, RESTRICT on
//! accounts used by bets, unique rate per account and day) so both builds behave the same.

use std::collections::HashMap;

use chrono::NaiveDate;
use fxbet_core::domain::fx::Currency;
use fxbet_core::domain::savings::Compounding;
use fxbet_core::model::{Account, AccountRate, Bet, ConversionRecord, DEFAULT_ACCOUNTS, DEFAULT_TAX_PCT, NewAccount, NewBet};
use fxbet_core::service::bets::NewConversion;
use fxbet_core::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    /// Last id handed out per record kind, like SQLite AUTOINCREMENT: never reused.
    last_ids: LastIds,
    accounts: Vec<AccountRow>,
    rates: Vec<AccountRate>,
    bets: Vec<Bet>,
    conversions: Vec<ConversionRecord>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LastIds {
    account: i64,
    rate: i64,
    bet: i64,
    conversion: i64,
}

fn next(last: &mut i64) -> i64 {
    *last += 1;
    *last
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountRow {
    id: i64,
    name: String,
    currency: Currency,
    compounding: Compounding,
    tax_pct: f64,
    created_at: String,
}

impl Store {
    /// Empty store with the same placeholder accounts the server seeds.
    pub fn seeded(today: NaiveDate) -> Self {
        let mut s = Store::default();
        let created_at = format!("{today}T00:00:00Z");
        for (name, currency, compounding, rate) in DEFAULT_ACCOUNTS {
            let id = s.insert_account(&NewAccount { name: name.into(), currency, compounding, tax_pct: DEFAULT_TAX_PCT }, &created_at);
            s.upsert_rate(id, today, rate);
        }
        s
    }

    // ---------- accounts ----------

    pub fn accounts(&self) -> Vec<Account> {
        self.accounts.iter().map(|a| self.assemble(a)).collect()
    }

    pub fn account(&self, id: i64) -> Result<Account> {
        self.accounts.iter().find(|a| a.id == id).map(|a| self.assemble(a)).ok_or(Error::NotFound("account"))
    }

    fn assemble(&self, a: &AccountRow) -> Account {
        let rates = self.rates.iter().filter(|r| r.account_id == a.id).cloned().collect();
        Account::assemble(a.id, a.name.clone(), a.currency, a.compounding, a.tax_pct, rates)
    }

    pub fn insert_account(&mut self, a: &NewAccount, now: &str) -> i64 {
        let id = next(&mut self.last_ids.account);
        self.accounts.push(AccountRow {
            id,
            name: a.name.clone(),
            currency: a.currency,
            compounding: a.compounding,
            tax_pct: a.tax_pct,
            created_at: now.to_string(),
        });
        id
    }

    pub fn update_account(&mut self, id: i64, a: &NewAccount) -> Result<()> {
        let row = self.accounts.iter_mut().find(|r| r.id == id).ok_or(Error::NotFound("account"))?;
        row.name = a.name.clone();
        row.currency = a.currency;
        row.compounding = a.compounding;
        row.tax_pct = a.tax_pct;
        Ok(())
    }

    pub fn delete_account(&mut self, id: i64) -> Result<()> {
        let used = self.bets.iter().filter(|b| [b.pln_account_id, b.usd_account_id, b.eur_account_id].contains(&id)).count();
        if used > 0 {
            return Err(Error::Conflict(format!("account={id} is used by {used} bet(s); delete those bets first")));
        }
        let before = self.accounts.len();
        self.accounts.retain(|a| a.id != id);
        if self.accounts.len() == before {
            return Err(Error::NotFound("account"));
        }
        self.rates.retain(|r| r.account_id != id);
        Ok(())
    }

    pub fn upsert_rate(&mut self, account_id: i64, from: NaiveDate, rate_pct: f64) {
        if let Some(r) = self.rates.iter_mut().find(|r| r.account_id == account_id && r.effective_from == from) {
            r.annual_rate_pct = rate_pct;
            return;
        }
        let id = next(&mut self.last_ids.rate);
        self.rates.push(AccountRate { id, account_id, effective_from: from, annual_rate_pct: rate_pct });
    }

    pub fn delete_rate(&mut self, account_id: i64, rate_id: i64) -> Result<()> {
        if self.rates.iter().filter(|r| r.account_id == account_id).count() <= 1 {
            return Err(Error::Conflict("an account needs at least one rate".into()));
        }
        let before = self.rates.len();
        self.rates.retain(|r| !(r.id == rate_id && r.account_id == account_id));
        if self.rates.len() == before { Err(Error::NotFound("rate")) } else { Ok(()) }
    }

    // ---------- bets ----------

    /// Newest start date first, like the server.
    pub fn bets(&self) -> Vec<Bet> {
        let mut bets = self.bets.clone();
        bets.sort_by(|a, b| b.start_date.cmp(&a.start_date).then(b.id.cmp(&a.id)));
        bets
    }

    pub fn bet(&self, id: i64) -> Result<Bet> {
        self.bets.iter().find(|b| b.id == id).cloned().ok_or(Error::NotFound("bet"))
    }

    pub fn insert_bet(&mut self, bet: NewBet, conversions: &[NewConversion], now: &str) -> i64 {
        let id = next(&mut self.last_ids.bet);
        self.bets.push(bet.into_bet(id, now.to_string()));
        for c in conversions {
            self.insert_conversion(id, c);
        }
        id
    }

    pub fn update_bet_notes(&mut self, id: i64, title: String, notes: String) -> Result<()> {
        let bet = self.bets.iter_mut().find(|b| b.id == id).ok_or(Error::NotFound("bet"))?;
        bet.title = title;
        bet.notes = notes;
        Ok(())
    }

    pub fn delete_bet(&mut self, id: i64) -> Result<()> {
        let before = self.bets.len();
        self.bets.retain(|b| b.id != id);
        if self.bets.len() == before {
            return Err(Error::NotFound("bet"));
        }
        self.conversions.retain(|c| c.bet_id != id);
        Ok(())
    }

    // ---------- conversions ----------

    /// One bet's conversions by date, then id.
    pub fn conversions(&self, bet_id: i64) -> Vec<ConversionRecord> {
        let mut v: Vec<_> = self.conversions.iter().filter(|c| c.bet_id == bet_id).cloned().collect();
        v.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
        v
    }

    pub fn conversions_by_bet(&self) -> HashMap<i64, Vec<ConversionRecord>> {
        let mut map: HashMap<i64, Vec<ConversionRecord>> = HashMap::new();
        for bet in &self.bets {
            map.insert(bet.id, self.conversions(bet.id));
        }
        map
    }

    pub fn insert_conversion(&mut self, bet_id: i64, c: &NewConversion) {
        let id = next(&mut self.last_ids.conversion);
        self.conversions.push(ConversionRecord { id, bet_id, date: c.date, usd_amount: c.usd_amount, rate: c.rate, note: c.note.clone() });
    }

    pub fn delete_conversion(&mut self, bet_id: i64, id: i64) -> Result<()> {
        let before = self.conversions.len();
        self.conversions.retain(|c| !(c.id == id && c.bet_id == bet_id));
        if self.conversions.len() == before { Err(Error::NotFound("conversion")) } else { Ok(()) }
    }
}
