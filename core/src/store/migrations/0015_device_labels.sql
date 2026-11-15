-- 0015: labels for devices on the network map that aren't hosts (M10i
-- follow-up). Keyed by MAC address (stable across DHCP), or "ip:<addr>"
-- when the scan saw no MAC. Config, so it is part of the encrypted export.

CREATE TABLE device_labels (
    id         INTEGER PRIMARY KEY,
    key        TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL DEFAULT '',
    kind       TEXT NOT NULL DEFAULT 'other',
    -- JSON array of strings
    tags       TEXT NOT NULL DEFAULT '[]',
    note       TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;
