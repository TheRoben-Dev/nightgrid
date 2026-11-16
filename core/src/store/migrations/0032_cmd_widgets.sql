-- 0032 (M10x): custom command widgets. A read-only command run on a host and
-- shown as a dashboard tile (kind "cmd:<id>"). Nothing here is secret.

CREATE TABLE cmd_widgets (
    id            INTEGER PRIMARY KEY,
    host_id       INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    command       TEXT NOT NULL,
    parse         TEXT NOT NULL DEFAULT 'text',
    pattern       TEXT NOT NULL DEFAULT '',
    unit          TEXT NOT NULL DEFAULT '',
    warn          REAL,
    crit          REAL,
    below         INTEGER NOT NULL DEFAULT 0,
    interval_secs INTEGER NOT NULL DEFAULT 60,
    created_at    INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;
