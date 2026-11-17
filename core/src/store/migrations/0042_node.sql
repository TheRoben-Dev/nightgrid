-- 0042: Nightgrid Node pairing (M12f). Which node a host had last time (a
-- new node id means its history started over) and whether to use it, plus
-- the sync cursors per history kind: 60/900/3600 for metric tiers, -1
-- events, -2 uptime hours, -3 disk usage. History, not configuration: not
-- in profile exports.
CREATE TABLE node_hosts (
    host_id  INTEGER PRIMARY KEY REFERENCES hosts(id) ON DELETE CASCADE,
    node_id  TEXT    NOT NULL DEFAULT '',
    use_node INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE node_sync (
    host_id INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    res     INTEGER NOT NULL,
    cursor  INTEGER NOT NULL,
    PRIMARY KEY (host_id, res)
) STRICT, WITHOUT ROWID;
