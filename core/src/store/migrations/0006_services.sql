-- 0006: services board (HTTP health checks) and their recent results.

CREATE TABLE services (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL,
    url             TEXT NOT NULL,
    -- Optional host the service runs on (grouping, "open through tunnel").
    host_id         INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
    group_name      TEXT NOT NULL DEFAULT '',
    description     TEXT NOT NULL DEFAULT '',
    icon            TEXT,
    check_enabled   INTEGER NOT NULL DEFAULT 1 CHECK (check_enabled IN (0, 1)),
    -- Status codes that count as up, e.g. "200-399" or "200,401".
    expect_status   TEXT NOT NULL DEFAULT '200-399',
    -- Self-signed/internal CA: report the cert but don't mark the service down.
    allow_untrusted INTEGER NOT NULL DEFAULT 0 CHECK (allow_untrusted IN (0, 1)),
    sort_order      INTEGER NOT NULL DEFAULT 0,
    -- 'manual' or 'homepage'; homepage entries are matched on re-sync by source_key.
    source          TEXT NOT NULL DEFAULT 'manual',
    source_key      TEXT,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at      INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE INDEX services_host_idx ON services(host_id);
CREATE UNIQUE INDEX services_source_key_idx ON services(source, source_key) WHERE source_key IS NOT NULL;

CREATE TABLE service_checks (
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    ts         INTEGER NOT NULL,          -- unix seconds
    up         INTEGER NOT NULL CHECK (up IN (0, 1)),
    status     INTEGER,                   -- HTTP status, NULL when no response
    latency_ms INTEGER,
    PRIMARY KEY (service_id, ts)
) STRICT, WITHOUT ROWID;
