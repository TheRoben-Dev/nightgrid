-- 0014: a note can be kept out of DRIFT's search (M10i).
ALTER TABLE notes ADD COLUMN hide_from_drift INTEGER NOT NULL DEFAULT 0;
