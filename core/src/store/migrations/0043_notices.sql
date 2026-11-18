-- 0043 (1.1): the notifications inbox. Every notice Nightgrid sends (alerts,
-- restore drills, rotations due, posture drops, capacity warnings) lands here
-- whether or not it was also shown on the desktop or sent to a self-hosted
-- ntfy. Titles and short bodies only; kept 30 days, at most 1000 rows.

CREATE TABLE notices (
    id      INTEGER PRIMARY KEY,
    at      INTEGER NOT NULL,
    source  TEXT NOT NULL,
    level   TEXT NOT NULL,
    title   TEXT NOT NULL,
    body    TEXT NOT NULL DEFAULT '',
    host_id INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
    view    TEXT,
    read    INTEGER NOT NULL DEFAULT 0 CHECK (read IN (0, 1))
) STRICT;

CREATE INDEX notices_at ON notices(at);
