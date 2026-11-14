-- 0005: metric history. One row per host, resolution and bucket start
-- (unix seconds): 60 s buckets for recent data, 900 s rollups for longer ranges.

CREATE TABLE metric_points (
    host_id    INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    res        INTEGER NOT NULL,
    ts         INTEGER NOT NULL,
    n          INTEGER NOT NULL,
    cpu        REAL,
    mem        REAL,
    swap       REAL,
    load1      REAL,
    disk       REAL,
    net_rx     REAL,
    net_tx     REAL,
    disk_read  REAL,
    disk_write REAL,
    temp       REAL,
    PRIMARY KEY (host_id, res, ts)
) STRICT, WITHOUT ROWID;
