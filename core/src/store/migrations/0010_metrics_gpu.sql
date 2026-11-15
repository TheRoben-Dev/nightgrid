-- 0010: GPU busy % (the busiest GPU) in metric history (M10c).
ALTER TABLE metric_points ADD COLUMN gpu REAL;
