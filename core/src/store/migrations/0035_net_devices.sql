-- 0035: network device inventory (network map). One row per device the map
-- has ever seen, keyed like device labels: the MAC, or ip:<address> when the
-- MAC can't be seen (routed ranges). Lets a scan say what's new and what's
-- missing, and holds the user's "trusted" mark.

CREATE TABLE net_devices (
    key TEXT PRIMARY KEY,
    ip TEXT NOT NULL,
    vendor TEXT,
    kind TEXT NOT NULL DEFAULT 'other',
    name TEXT,
    -- Where it was last seen from: a host id, or 'local'.
    origin TEXT NOT NULL,
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    trusted INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;
