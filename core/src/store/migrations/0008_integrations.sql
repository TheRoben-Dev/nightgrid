-- 0008: integrations (Plex, AMP, Uptime Kuma, Portainer). Tokens and
-- passwords live in the OS keychain, never here.

CREATE TABLE integrations (
    id              INTEGER PRIMARY KEY,
    kind            TEXT NOT NULL CHECK (kind IN ('plex', 'amp', 'kuma', 'portainer')),
    name            TEXT NOT NULL DEFAULT '',
    base_url        TEXT NOT NULL,
    -- AMP login name.
    username        TEXT NOT NULL DEFAULT '',
    -- Uptime Kuma status page slug.
    slug            TEXT NOT NULL DEFAULT '',
    -- Portainer environment (endpoint) id for deep links.
    endpoint_id     INTEGER NOT NULL DEFAULT 0 CHECK (endpoint_id >= 0),
    -- Portainer: the host whose Docker this environment manages.
    host_id         INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
    allow_untrusted INTEGER NOT NULL DEFAULT 0 CHECK (allow_untrusted IN (0, 1)),
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;
