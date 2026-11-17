-- 0038: a client host (laptop, handheld, desktop) is expected to come and go,
-- so it shows "Away" instead of "Offline" and raises no offline alert.
-- alert_offline = 1 opts a client back into offline alerts. Servers always
-- alert; the flag is ignored for them.

ALTER TABLE hosts ADD COLUMN alert_offline INTEGER NOT NULL DEFAULT 0;
