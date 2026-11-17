-- 0041: power profiles (M12d). What a host draws at idle and at full CPU,
-- typed in by the user or picked from a preset; estimates are worked out
-- from these and the hourly metric tier, nothing else is stored.
CREATE TABLE host_power (
    host_id INTEGER PRIMARY KEY REFERENCES hosts(id) ON DELETE CASCADE,
    idle_w  REAL NOT NULL,
    max_w   REAL NOT NULL
);
