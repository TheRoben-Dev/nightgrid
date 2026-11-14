-- 0007: saved port forwards per host.

CREATE TABLE tunnels (
    id          INTEGER PRIMARY KEY,
    host_id     INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    name        TEXT NOT NULL DEFAULT '',
    -- 'local' (-L), 'remote' (-R) or 'dynamic' (-D, SOCKS5).
    kind        TEXT NOT NULL CHECK (kind IN ('local', 'remote', 'dynamic')),
    -- Where to listen: this machine for local/dynamic, the server for remote.
    bind_addr   TEXT NOT NULL,
    bind_port   INTEGER NOT NULL CHECK (bind_port BETWEEN 0 AND 65535),
    -- Where connections go: seen from the server for local, from this machine for remote. Unused for dynamic.
    dest_host   TEXT NOT NULL DEFAULT '',
    dest_port   INTEGER NOT NULL DEFAULT 0 CHECK (dest_port BETWEEN 0 AND 65535),
    auto_start  INTEGER NOT NULL DEFAULT 0 CHECK (auto_start IN (0, 1)),
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE INDEX tunnels_host_idx ON tunnels(host_id);
