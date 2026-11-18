-- 0044 (1.1): security posture runs. The System → security audit plus two
-- extra checks, scored per host each time it runs (by hand or once a day on
-- connected hosts), so the Security view can show a trend and a drop can
-- notify. Check titles, states and short details only; 90 runs per host.

CREATE TABLE posture_runs (
    id         INTEGER PRIMARY KEY,
    host_id    INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    at         INTEGER NOT NULL,
    score      INTEGER,
    grade      TEXT,
    checks     TEXT NOT NULL DEFAULT '[]',
    open_ports INTEGER NOT NULL DEFAULT 0,
    error      TEXT
) STRICT;

CREATE INDEX posture_runs_host ON posture_runs(host_id, at);
