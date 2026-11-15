-- 0020 (M10m): disk usage trend for the "full in about X days" forecast.
-- One row per host, mount and hour (the last sample of that hour), kept 90
-- days. History, not config: not part of the export.

CREATE TABLE disk_usage (
    host_id INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    mount   TEXT    NOT NULL,
    ts      INTEGER NOT NULL,
    used    INTEGER NOT NULL,
    size    INTEGER NOT NULL,
    PRIMARY KEY (host_id, mount, ts)
) STRICT, WITHOUT ROWID;
