-- 0045 (1.1): secrets & rotation. Dates and policies only, never values:
-- `secret_meta` is keyed by a stable id (`ssh:<fingerprint>`, `vault:<id>`,
-- `app:mcp` …) with when Nightgrid last set or the user last marked it
-- rotated, its rotation policy in days (0 = none, NULL = the default) and
-- the due date already notified. `manual_secrets` lists secrets kept outside
-- Nightgrid by name and place.

CREATE TABLE secret_meta (
    key          TEXT PRIMARY KEY,
    rotated_at   INTEGER,
    policy_days  INTEGER,
    notified_due INTEGER
) STRICT;

CREATE TABLE manual_secrets (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    place      TEXT NOT NULL DEFAULT '',
    host_id    INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
    rotated_at INTEGER,
    note       TEXT NOT NULL DEFAULT ''
) STRICT;
