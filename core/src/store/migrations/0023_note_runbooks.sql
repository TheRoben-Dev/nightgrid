-- 0023 (M10s): a note can be a runbook for alerts. It applies to the alert
-- kinds listed (JSON array; empty = any kind), narrowed to its host and to a
-- service when those are set, and is offered on matching alerts.
ALTER TABLE notes ADD COLUMN runbook INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN alert_kinds TEXT NOT NULL DEFAULT '[]';
ALTER TABLE notes ADD COLUMN service_id INTEGER REFERENCES services(id) ON DELETE SET NULL;
