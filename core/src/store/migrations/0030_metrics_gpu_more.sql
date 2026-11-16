-- 0030 (M10x): more GPU history: the fullest VRAM (%), the hottest GPU (°C)
-- and the power of all GPUs together (W). Older points keep NULL.
ALTER TABLE metric_points ADD COLUMN gpu_mem REAL;
ALTER TABLE metric_points ADD COLUMN gpu_temp REAL;
ALTER TABLE metric_points ADD COLUMN gpu_power REAL;
