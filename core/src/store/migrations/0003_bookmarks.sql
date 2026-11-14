-- 0003: per-host path bookmarks for the file browser.
-- host_id NULL means a local (this machine) bookmark.

CREATE TABLE bookmarks (
    id         INTEGER PRIMARY KEY,
    host_id    INTEGER REFERENCES hosts(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    label      TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE UNIQUE INDEX bookmarks_host_path_idx ON bookmarks(ifnull(host_id, 0), path);
