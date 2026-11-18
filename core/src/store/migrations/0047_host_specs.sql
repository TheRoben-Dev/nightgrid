-- 0047 (1.1): capacity planner. Each host's size (cores, memory) from its
-- latest metrics sample, so Fleet → CAPACITY works while a host is offline,
-- and when a capacity notice last went out for it.

CREATE TABLE host_specs (
    host_id     INTEGER PRIMARY KEY REFERENCES hosts(id) ON DELETE CASCADE,
    cores       INTEGER NOT NULL,
    mem_total   INTEGER NOT NULL,
    at          INTEGER NOT NULL,
    notified_at INTEGER
) STRICT;
