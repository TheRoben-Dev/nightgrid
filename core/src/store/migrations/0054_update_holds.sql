-- 0054: update holds (D-195). Things the user keeps out of updates: a whole
-- host (for a while or until released), a package where the host can't hold
-- it itself (pacman, dnf without versionlock), a container image in a stack,
-- or a whole stack. Held things aren't counted as pending, aren't selected
-- for batch upgrades and aren't pulled. Packages the host holds natively
-- (apt-mark hold, dnf versionlock, hidden Windows updates) live on the host,
-- not here. Config, so part of the export.

CREATE TABLE update_holds (
    id         INTEGER PRIMARY KEY,
    host_id    INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL CHECK (kind IN ('host', 'package', 'image', 'stack')),
    -- Package name, image reference or stack name; '' for a host.
    target     TEXT NOT NULL DEFAULT '',
    -- The stack an image belongs to; '' otherwise.
    stack      TEXT NOT NULL DEFAULT '',
    -- What was installed or running when it was held (shown, not enforced).
    version    TEXT,
    -- Unix seconds the hold ends by itself; NULL = until released.
    until      INTEGER,
    note       TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (host_id, kind, stack, target)
) STRICT;
