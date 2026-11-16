-- 0029 (M10w): compose stacks tracked in a git repository. The checkout lives
-- on the host at `dir`; git runs there with the host's own credentials, so
-- nothing here is secret. `subdir` is the stack's folder inside the repo ("" =
-- the root). `deployed` / `previous` are the commits of the last two deploys.

CREATE TABLE git_stacks (
    id          INTEGER PRIMARY KEY,
    host_id     INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    dir         TEXT NOT NULL,
    url         TEXT NOT NULL,
    branch      TEXT NOT NULL,
    subdir      TEXT NOT NULL DEFAULT '',
    deployed    TEXT,
    previous    TEXT,
    deployed_at INTEGER,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (host_id, dir, subdir)
) STRICT;
