-- 0037: per-host opt-in for DRIFT's read-only checks (D-151), and extra
-- folders it may read files from on that host (one per line).

ALTER TABLE hosts ADD COLUMN drift_lookups INTEGER NOT NULL DEFAULT 0;
ALTER TABLE hosts ADD COLUMN drift_paths TEXT;
