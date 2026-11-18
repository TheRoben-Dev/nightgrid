-- 0046 (1.1): restore drills. A drill is a backup job's test restore, or a
-- Proxmox guest's newest backup restored into an isolated, short-lived
-- sandbox, run by hand or every N days. Runs keep what happened step by step
-- (no file contents, no secrets), 50 per drill.

CREATE TABLE drills (
    id             INTEGER PRIMARY KEY,
    name           TEXT NOT NULL,
    kind           TEXT NOT NULL CHECK (kind IN ('job', 'proxmox')),
    host_id        INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    job_id         INTEGER REFERENCES backup_jobs(id) ON DELETE CASCADE,
    node           TEXT,
    guest_kind     TEXT,
    vmid           INTEGER,
    storage        TEXT,
    target_storage TEXT,
    every_days     INTEGER NOT NULL DEFAULT 0,
    enabled        INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1))
) STRICT;

CREATE TABLE drill_runs (
    id       INTEGER PRIMARY KEY,
    drill_id INTEGER NOT NULL REFERENCES drills(id) ON DELETE CASCADE,
    at       INTEGER NOT NULL,
    seconds  INTEGER NOT NULL,
    ok       INTEGER NOT NULL CHECK (ok IN (0, 1)),
    steps    TEXT NOT NULL,
    error    TEXT,
    detail   TEXT NOT NULL DEFAULT ''
) STRICT;

CREATE INDEX drill_runs_by_drill ON drill_runs (drill_id, at DESC);
