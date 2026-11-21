-- 0058: devices paired for profile sync (D-205). State of this computer:
-- never part of a profile export. The device's own key lives in the keychain.

CREATE TABLE sync_peers (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    public_key     TEXT NOT NULL UNIQUE,
    addrs          TEXT NOT NULL DEFAULT '[]',
    port           INTEGER NOT NULL,
    mode           TEXT NOT NULL DEFAULT 'manual',
    share_groups  TEXT NOT NULL DEFAULT '[]',
    accept_changes INTEGER NOT NULL DEFAULT 1,
    app_version    TEXT NOT NULL DEFAULT '',
    paired_at      INTEGER NOT NULL,
    last_sync      INTEGER,
    last_seen      INTEGER,
    last_error     TEXT
) STRICT;

-- Automatic sync: one row per synced item (a config row, a host tag, an app
-- setting) with a stable id shared by every device, the hash of its
-- content and when it last changed (unix ms). Deleted items stay as
-- tombstones for a while so a device that was offline learns about them.
CREATE TABLE sync_items (
    tbl       TEXT NOT NULL,
    uid       TEXT NOT NULL,
    local_id  INTEGER,
    local_key TEXT,
    hash      TEXT NOT NULL,
    modified  INTEGER NOT NULL,
    deleted   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (tbl, uid)
) STRICT;
CREATE UNIQUE INDEX sync_items_id ON sync_items (tbl, local_id) WHERE local_id IS NOT NULL;
CREATE UNIQUE INDEX sync_items_key ON sync_items (tbl, local_key) WHERE local_key IS NOT NULL;
