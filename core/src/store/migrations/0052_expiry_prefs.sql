-- 0052: Expiry Watch edits for rows Nightgrid finds by itself
-- (service and proxy route certificates), and muting any row.
-- A trusted fingerprint accepts one self-signed or private-CA certificate:
-- a different certificate warns again. Config, so part of the export.

ALTER TABLE watch_items ADD COLUMN muted INTEGER NOT NULL DEFAULT 0;

CREATE TABLE expiry_prefs (
    id                  INTEGER PRIMARY KEY,
    service_id          INTEGER REFERENCES services(id) ON DELETE CASCADE,
    proxy_domain        TEXT,
    trusted_fingerprint TEXT,
    muted               INTEGER NOT NULL DEFAULT 0,
    expires_at          INTEGER,
    warn_days           INTEGER,
    note                TEXT NOT NULL DEFAULT '',
    CHECK ((service_id IS NULL) <> (proxy_domain IS NULL))
) STRICT;

CREATE UNIQUE INDEX expiry_prefs_service ON expiry_prefs(service_id) WHERE service_id IS NOT NULL;
CREATE UNIQUE INDEX expiry_prefs_proxy ON expiry_prefs(proxy_domain) WHERE proxy_domain IS NOT NULL;
