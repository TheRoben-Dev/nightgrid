-- 0036: the network map as the user arranged it: node positions, the parent
-- a device connects through (set by hand), and which nodes are expected
-- ("save as expected"), so a scan can show what changed. Keys: a device's
-- MAC or ip:<address>, host:<name> for hosts, or gw / ts / net / local.
-- Config, so it is part of the encrypted export.

CREATE TABLE netmap_layout (
    id         INTEGER PRIMARY KEY,
    key        TEXT NOT NULL UNIQUE,
    x          REAL,
    y          REAL,
    parent     TEXT,
    expected   INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;
