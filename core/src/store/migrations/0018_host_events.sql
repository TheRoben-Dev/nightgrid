-- 0018 (M10l): per-host incident timeline. What Nightgrid saw or did on a
-- host: alerts firing and clearing, power actions, compose runs, rollbacks,
-- unit actions, upgrades and automation runs. Titles and short details only
-- (secret-looking text is dropped before it gets here, rule 1). Kept 90 days,
-- at most 5000 rows per host. History, not config: not part of exports.

CREATE TABLE host_events (
    id      INTEGER PRIMARY KEY,
    host_id INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    at      INTEGER NOT NULL,
    kind    TEXT NOT NULL,
    level   TEXT NOT NULL DEFAULT 'info',
    title   TEXT NOT NULL,
    detail  TEXT NOT NULL DEFAULT ''
) STRICT;

CREATE INDEX host_events_host ON host_events(host_id, at);
