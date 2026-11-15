-- 0009: automation (M8): snippets and playbooks. No secrets here: a sudo
-- password for a run is typed per run (or comes from the per-host keychain
-- entry) and never stored with the snippet.

CREATE TABLE snippets (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    command     TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    scope       TEXT NOT NULL DEFAULT 'global' CHECK (scope IN ('global', 'host', 'group')),
    host_id     INTEGER REFERENCES hosts(id) ON DELETE CASCADE,
    group_id    INTEGER REFERENCES host_groups(id) ON DELETE CASCADE,
    sudo        INTEGER NOT NULL DEFAULT 0 CHECK (sudo IN (0, 1)),
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE TABLE playbooks (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    -- JSON array of steps (see core/src/automation/mod.rs `Step`).
    steps         TEXT NOT NULL DEFAULT '[]',
    stop_on_error INTEGER NOT NULL DEFAULT 1 CHECK (stop_on_error IN (0, 1)),
    created_at    INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at    INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;
