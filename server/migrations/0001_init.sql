-- Dates are civil dates (YYYY-MM-DD) of FX fixings / bank value dates; timestamps are UTC RFC3339.

CREATE TABLE fx_rates (
    date      TEXT PRIMARY KEY NOT NULL,
    usd_pln   REAL NOT NULL CHECK (usd_pln > 0),
    usd_eur   REAL NOT NULL CHECK (usd_eur > 0),
    source    TEXT NOT NULL
);

CREATE TABLE accounts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    currency    TEXT NOT NULL CHECK (currency IN ('USD', 'EUR', 'PLN')),
    compounding TEXT NOT NULL CHECK (compounding IN ('daily', 'monthly')),
    tax_pct     REAL NOT NULL CHECK (tax_pct >= 0 AND tax_pct < 100),
    created_at  TEXT NOT NULL
);

CREATE TABLE account_rates (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id       INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    effective_from   TEXT NOT NULL,
    annual_rate_pct  REAL NOT NULL CHECK (annual_rate_pct >= 0),
    UNIQUE (account_id, effective_from)
);

CREATE TABLE bets (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at       TEXT NOT NULL,
    title            TEXT NOT NULL,
    notes            TEXT NOT NULL DEFAULT '',
    amount_usd       REAL NOT NULL CHECK (amount_usd > 0),
    start_date       TEXT NOT NULL,
    deadline         TEXT NOT NULL,
    spot_usd_pln     REAL NOT NULL CHECK (spot_usd_pln > 0),
    spot_usd_eur     REAL NOT NULL CHECK (spot_usd_eur > 0),
    target_rate      REAL NOT NULL CHECK (target_rate > 0),
    spread_pct       REAL NOT NULL CHECK (spread_pct >= 0 AND spread_pct < 100),
    subjective_prob  REAL CHECK (subjective_prob IS NULL OR (subjective_prob >= 0 AND subjective_prob <= 1)),
    model_prob       REAL NOT NULL CHECK (model_prob >= 0 AND model_prob <= 1),
    pln_account_id   INTEGER NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    usd_account_id   INTEGER NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    eur_account_id   INTEGER NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    -- Inputs and headline numbers as seen when the bet was placed; never recomputed.
    snapshot         TEXT NOT NULL
);

CREATE TABLE conversions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    bet_id      INTEGER NOT NULL REFERENCES bets(id) ON DELETE CASCADE,
    date        TEXT NOT NULL,
    usd_amount  REAL NOT NULL CHECK (usd_amount > 0),
    rate        REAL NOT NULL CHECK (rate > 0),
    note        TEXT NOT NULL DEFAULT ''
);

CREATE INDEX conversions_bet ON conversions(bet_id);
