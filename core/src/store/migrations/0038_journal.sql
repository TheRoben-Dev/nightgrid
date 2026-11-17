-- 0038: the homelab journal. Every change made from Nightgrid (deploys,
-- config and file edits, firewall, cron, proxy routes, updates, power,
-- Proxmox, keys) plus entries the user writes by hand, across all hosts.
-- Titles, short details and, for text edits, a line diff whose
-- secret-looking lines are hidden before they get here (rule 1). Kept two
-- years, at most 50 000 rows. History, not config: not part of exports.

CREATE TABLE journal (
    id      INTEGER PRIMARY KEY,
    at      INTEGER NOT NULL,
    host_id INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
    -- Kept so an entry still names its host after the host is removed.
    host    TEXT NOT NULL DEFAULT '',
    area    TEXT NOT NULL,
    title   TEXT NOT NULL,
    detail  TEXT NOT NULL DEFAULT '',
    diff    TEXT,
    ok      INTEGER NOT NULL DEFAULT 1,
    manual  INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX journal_at ON journal(at);
CREATE INDEX journal_host ON journal(host_id, at);
