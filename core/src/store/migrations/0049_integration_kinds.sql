-- 0049: allow every integration kind. 0008's CHECK only listed plex, amp,
-- kuma and portainer, so adding Pi-hole or AdGuard Home (0.10.36) failed.
-- SQLite can't alter a CHECK constraint, so the table is rebuilt with the
-- same columns and rows (ids kept, keychain entries stay valid).

CREATE TABLE integrations_new (
    id              INTEGER PRIMARY KEY,
    kind            TEXT NOT NULL CHECK (kind IN ('plex', 'amp', 'kuma', 'portainer', 'pihole', 'adguard')),
    name            TEXT NOT NULL DEFAULT '',
    base_url        TEXT NOT NULL,
    -- AMP and AdGuard Home login name.
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

INSERT INTO integrations_new (id, kind, name, base_url, username, slug, endpoint_id, host_id, allow_untrusted, created_at)
    SELECT id, kind, name, base_url, username, slug, endpoint_id, host_id, allow_untrusted, created_at FROM integrations;

DROP TABLE integrations;
ALTER TABLE integrations_new RENAME TO integrations;
