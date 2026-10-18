use chrono::{NaiveDate, Utc};

/// Source of "today". Injected so handlers and tests don't depend on wall-clock time.
pub trait Clock: Send + Sync {
    fn today(&self) -> NaiveDate;
}

pub struct SystemClock;

impl Clock for SystemClock {
    // ECB fixings are dated in CET; UTC date differs only between 23:00 and 01:00 CET,
    // when no new fixing is published anyway.
    fn today(&self) -> NaiveDate {
        Utc::now().date_naive()
    }
}
