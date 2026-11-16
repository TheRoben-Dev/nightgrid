-- 0022 (M10s): what depends on what, for folding alerts under their cause.
-- Each side is a host or a service (exactly one of the two columns set).
-- Config, so part of the export; rows go when either end is deleted.

CREATE TABLE alert_deps (
    id                INTEGER PRIMARY KEY,
    child_host_id     INTEGER REFERENCES hosts(id) ON DELETE CASCADE,
    child_service_id  INTEGER REFERENCES services(id) ON DELETE CASCADE,
    parent_host_id    INTEGER REFERENCES hosts(id) ON DELETE CASCADE,
    parent_service_id INTEGER REFERENCES services(id) ON DELETE CASCADE,
    CHECK ((child_host_id IS NULL) != (child_service_id IS NULL)),
    CHECK ((parent_host_id IS NULL) != (parent_service_id IS NULL))
) STRICT;
