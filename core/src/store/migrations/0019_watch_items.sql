-- 0019 (M10l): expiry watch. Certificate files on a host, domains, tokens,
-- licences: a name, where to read the date (a path or a domain) and the
-- date. Never the token itself (rule 1). Config, so part of the export.

CREATE TABLE watch_items (
    id          INTEGER PRIMARY KEY,
    kind        TEXT NOT NULL DEFAULT 'other',
    name        TEXT NOT NULL,
    host_id     INTEGER REFERENCES hosts(id) ON DELETE CASCADE,
    target      TEXT NOT NULL DEFAULT '',
    expires_at  INTEGER,
    warn_days   INTEGER NOT NULL DEFAULT 30,
    note        TEXT NOT NULL DEFAULT '',
    checked_at  INTEGER,
    check_error TEXT,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;
