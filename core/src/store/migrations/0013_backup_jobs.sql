-- 0013: backup status board (M10g). A job points at a systemd unit or a path
-- on the host; Nightgrid reads its status and never touches the repository.
-- Config, so it is part of the encrypted export.

CREATE TABLE backup_jobs (
    id          INTEGER PRIMARY KEY,
    host_id     INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    tool        TEXT NOT NULL DEFAULT 'other',
    -- 'unit' or 'path'
    source      TEXT NOT NULL DEFAULT 'unit',
    target      TEXT NOT NULL,
    stale_hours INTEGER NOT NULL DEFAULT 26,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE INDEX backup_jobs_host ON backup_jobs(host_id);
