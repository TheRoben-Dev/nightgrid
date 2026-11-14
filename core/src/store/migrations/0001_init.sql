-- 0001: initial schema.
-- Secrets (passwords, passphrases, tokens) are NEVER stored here; they live in
-- the OS keychain and are referenced by id. See core/src/secrets.rs.

CREATE TABLE settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,              -- JSON
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE TABLE host_groups (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    color      TEXT,
    icon       TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE TABLE hosts (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    hostname      TEXT NOT NULL,
    port          INTEGER NOT NULL DEFAULT 22 CHECK (port BETWEEN 1 AND 65535),
    username      TEXT,
    group_id      INTEGER REFERENCES host_groups(id) ON DELETE SET NULL,
    color         TEXT,
    icon          TEXT,
    favorite      INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1)),
    notes         TEXT,
    sort_order    INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at    INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE INDEX hosts_group_idx ON hosts(group_id);

CREATE TABLE tags (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL UNIQUE COLLATE NOCASE,
    color TEXT
) STRICT;

CREATE TABLE host_tags (
    host_id INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (host_id, tag_id)
) STRICT;
