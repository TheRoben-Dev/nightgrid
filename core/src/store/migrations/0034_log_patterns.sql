-- 0034: log error patterns seen per source (M10g guardrails), so a pattern
-- seen weeks ago isn't "new" after the app restarts. Only a hash of the
-- pattern is stored, never the line or the pattern text.

CREATE TABLE log_patterns (
    host_id INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    first_seen INTEGER NOT NULL,
    PRIMARY KEY (host_id, source, fingerprint)
) WITHOUT ROWID;
