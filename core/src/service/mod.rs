//! Endpoint logic without transport or storage: validate requests, resolve defaults from the
//! loaded records, run the model and shape responses.

pub mod accounts;
pub mod analyze;
pub mod bets;
pub mod fx;

use crate::error::{Result, bad};

pub(crate) fn positive(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && v > 0.0 { Ok(v) } else { Err(bad(format!("{name} must be a positive number"))) }
}

pub(crate) fn pct(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && (0.0..100.0).contains(&v) { Ok(v) } else { Err(bad(format!("{name} must be in [0, 100)"))) }
}

pub(crate) fn prob(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && (0.0..=1.0).contains(&v) { Ok(v) } else { Err(bad(format!("{name} must be in [0, 1]"))) }
}

pub(crate) fn non_empty(name: &str, v: &str) -> Result<String> {
    let t = v.trim();
    if t.is_empty() || t.len() > 200 { Err(bad(format!("{name} must be 1-200 characters"))) } else { Ok(t.to_string()) }
}
