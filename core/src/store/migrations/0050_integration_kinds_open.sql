-- 0050: drop the CHECK on integrations.kind. Each new integration kind used
-- to need a table rebuild (0049 fixed Pi-hole and AdGuard Home missing from
-- it). The kind is now validated in Rust (IntegrationKind::parse), so adding
-- a kind is a code change only. Same columns, rows and ids.

CREATE TABLE integrations_new (
    id              INTEGER PRIMARY KEY,
    kind            TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 32),
    name            TEXT NOT NULL DEFAULT '',
    base_url        TEXT NOT NULL,
    -- Login name, API key id or token id, depending on the kind.
    username        TEXT NOT NULL DEFAULT '',
    -- Uptime Kuma status page slug; UniFi site and similar per-kind extras.
    slug            TEXT NOT NULL DEFAULT '',
    -- Portainer environment (endpoint) id for deep links.
    endpoint_id     INTEGER NOT NULL DEFAULT 0 CHECK (endpoint_id >= 0),
    -- Portainer: the host whose Docker this environment manages.
    host_id         INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
    allow_untrusted INTEGER NOT NULL DEFAULT 0 CHECK (allow_untrusted IN (0, 1)),
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

INSERT INTO integrations_new (id, kind, name, base_url, username, slug, endpoint_id, host_id, allow_untrusted, created_at)
    SELECT id, kind, name, base_url, username, slug, endpoint_id, host_id, allow_untrusted, created_at FROM integrations;

DROP TABLE integrations;
ALTER TABLE integrations_new RENAME TO integrations;
