-- A bet recorded after the fact. Its subjective probability is hindsight-prone, so calibration ignores it.
ALTER TABLE bets ADD COLUMN retrospective INTEGER NOT NULL DEFAULT 0 CHECK (retrospective IN (0, 1));
