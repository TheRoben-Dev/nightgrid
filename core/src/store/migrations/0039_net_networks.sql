-- 0039: saved networks (network map). Each scan is matched to a network by
-- its fingerprint: the scanned subnet plus the router's address and MAC, so
-- two networks that both use 192.168.1.0/24 stay apart. Devices remember
-- the network they were last seen on, so a scan of one network never calls
-- another network's devices missing. The last scan of each network is kept
-- so the map can show it, or overlay several networks, without a rescan.

CREATE TABLE net_networks (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    subnet      TEXT,
    gateway     TEXT,
    gateway_mac TEXT,
    -- Where it was last scanned from: a host id, or 'local'.
    origin      TEXT NOT NULL,
    first_seen  INTEGER NOT NULL,
    last_seen   INTEGER NOT NULL,
    -- The last scan (JSON), and when it ran.
    snapshot    TEXT,
    snapshot_at INTEGER
) STRICT;

ALTER TABLE net_devices ADD COLUMN network INTEGER;
