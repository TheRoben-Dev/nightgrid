-- 0056: acknowledged and muted alerts (D-198). State of this computer, like
-- the inbox, so not part of a profile export. Timeline entries recorded
-- while an alert was silenced say so.

CREATE TABLE alert_silences (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    mode        TEXT NOT NULL CHECK (mode IN ('ack', 'mute')),
    key         TEXT,
    kind        TEXT,
    host_id     INTEGER REFERENCES hosts(id) ON DELETE CASCADE,
    label       TEXT NOT NULL DEFAULT '',
    severity    TEXT,
    until       INTEGER,
    until_clear INTEGER NOT NULL DEFAULT 0,
    created     INTEGER NOT NULL,
    CHECK (key IS NOT NULL OR kind IS NOT NULL OR host_id IS NOT NULL)
) STRICT;

ALTER TABLE host_events ADD COLUMN silenced TEXT;
