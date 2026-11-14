-- 0004: user-defined compose stack folders per host (scanned for compose files).

CREATE TABLE stack_dirs (
    id         INTEGER PRIMARY KEY,
    host_id    INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE UNIQUE INDEX stack_dirs_host_path_idx ON stack_dirs(host_id, path);
