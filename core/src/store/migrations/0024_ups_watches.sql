-- 0024 (M10s): UPS watches. A NUT name read with upsc on a host, and what
-- to do when it runs on battery: alerts, and optionally shutting a group of
-- hosts down after a countdown. Config, so part of the export.

CREATE TABLE ups_watches (
    id               INTEGER PRIMARY KEY,
    label            TEXT NOT NULL,
    target           TEXT NOT NULL,
    host_id          INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    group_id         INTEGER REFERENCES host_groups(id) ON DELETE SET NULL,
    shutdown         INTEGER NOT NULL DEFAULT 0,
    on_battery_secs  INTEGER NOT NULL DEFAULT 300,
    low_runtime_secs INTEGER NOT NULL DEFAULT 300,
    countdown_secs   INTEGER NOT NULL DEFAULT 60,
    enabled          INTEGER NOT NULL DEFAULT 1
) STRICT;
