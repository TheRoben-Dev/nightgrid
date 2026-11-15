-- 0012: stack snapshots (M10f). Which image each service of a compose
-- project ran, so a stack can be rolled back after an update. Image ids and
-- registry digests only here; copies of the compose and .env files are kept
-- on the host itself (they can hold secrets, which never come to this
-- database). Host-specific, so not part of backups.

CREATE TABLE stack_snapshots (
    id          INTEGER PRIMARY KEY,
    host_id     INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    project     TEXT NOT NULL,
    reason      TEXT NOT NULL DEFAULT 'manual',
    -- JSON array of {service, image, imageId, repoDigest}.
    services    TEXT NOT NULL DEFAULT '[]',
    -- JSON array of {path, saved}: files copied on the host, and where.
    files       TEXT NOT NULL DEFAULT '[]',
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE INDEX stack_snapshots_project ON stack_snapshots(host_id, project, created_at);
