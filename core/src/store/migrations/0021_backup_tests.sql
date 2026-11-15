-- 0021 (M10m): the last test restore of each backup job: when, whether it
-- passed and a short note (the file restored, or why it failed).

ALTER TABLE backup_jobs ADD COLUMN tested_at INTEGER;
ALTER TABLE backup_jobs ADD COLUMN test_ok INTEGER;
ALTER TABLE backup_jobs ADD COLUMN test_note TEXT;
