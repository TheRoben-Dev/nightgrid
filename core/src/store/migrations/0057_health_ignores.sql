-- 0057: containers whose healthcheck is ignored (D-201). A container that is
-- "running but unhealthy" by design no longer counts as down: no CONTAINER
-- DOWN alert, no red/amber status, no "unhealthy" in DRIFT's context or the
-- log watch. Crashes, exits and restart loops still alert. Keyed by host
-- plus compose project/service, or by container name for plain containers,
-- so it survives the container being recreated. Config, so part of the
-- export.

CREATE TABLE health_ignores (
    id         INTEGER PRIMARY KEY,
    host_id    INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    -- Compose project and service; both '' for a plain container.
    project    TEXT NOT NULL DEFAULT '',
    service    TEXT NOT NULL DEFAULT '',
    -- Container name; '' when matched by project/service.
    name       TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (host_id, project, service, name)
) STRICT;
