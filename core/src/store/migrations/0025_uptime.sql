-- 0025 (M10t): uptime history. Seconds seen up, down or asleep per host or
-- service per hour, and outages (start/end of each down spell). Only what
-- Nightgrid observed while running: gaps are unknown, not up. Kept 400
-- days. History, not config: not part of exports. Rows for removed hosts
-- and services are dropped by the daily prune (subjects are of two kinds,
-- so there is no foreign key).

CREATE TABLE uptime_hours (
    kind    TEXT NOT NULL CHECK (kind IN ('host', 'service')),
    subject INTEGER NOT NULL,
    hour    INTEGER NOT NULL,
    up      INTEGER NOT NULL DEFAULT 0,
    down    INTEGER NOT NULL DEFAULT 0,
    asleep  INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (kind, subject, hour)
) STRICT, WITHOUT ROWID;

CREATE TABLE outages (
    id      INTEGER PRIMARY KEY,
    kind    TEXT NOT NULL CHECK (kind IN ('host', 'service')),
    subject INTEGER NOT NULL,
    start   INTEGER NOT NULL,
    end     INTEGER NOT NULL,
    detail  TEXT NOT NULL DEFAULT ''
) STRICT;

CREATE INDEX outages_subject ON outages(kind, subject, start);
