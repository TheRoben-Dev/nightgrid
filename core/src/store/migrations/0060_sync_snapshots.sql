-- 0060: rolling snapshots of the synced profile, taken before changes from
-- another device land (D-209), so a bad change that synced over can be
-- rolled back. State of this computer: never part of a profile export.

CREATE TABLE sync_snapshots (
    id        INTEGER PRIMARY KEY,
    taken_at  INTEGER NOT NULL,
    reason    TEXT NOT NULL,
    peer      TEXT,
    -- sync_state 'generation' when taken: item ids stay valid within one.
    gen       INTEGER NOT NULL,
    counts    TEXT NOT NULL DEFAULT '{}',
    -- gzip JSON: the synced items (merge::Item, live ones only).
    items     BLOB NOT NULL,
    -- gzip JSON profile bundle, kept for snapshots taken before a whole
    -- profile arrived (that resets item ids).
    bundle    BLOB
) STRICT;

CREATE TABLE sync_state (
    key   TEXT PRIMARY KEY,
    value INTEGER NOT NULL
) STRICT;
