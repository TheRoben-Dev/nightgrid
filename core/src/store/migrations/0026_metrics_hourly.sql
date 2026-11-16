-- 0026 (M10t): hourly metric rollups (res 3600), kept 400 days, for ranges
-- longer than a month. Seeded from the 15-minute rollups already stored,
-- weighted by their sample counts (NULLs don't count).

INSERT INTO metric_points (host_id, res, ts, n, cpu, mem, swap, load1, disk, net_rx, net_tx, disk_read, disk_write, temp, gpu)
SELECT host_id, 3600, (ts / 3600) * 3600, SUM(n),
    SUM(cpu * n) / SUM(CASE WHEN cpu IS NOT NULL THEN n END),
    SUM(mem * n) / SUM(CASE WHEN mem IS NOT NULL THEN n END),
    SUM(swap * n) / SUM(CASE WHEN swap IS NOT NULL THEN n END),
    SUM(load1 * n) / SUM(CASE WHEN load1 IS NOT NULL THEN n END),
    SUM(disk * n) / SUM(CASE WHEN disk IS NOT NULL THEN n END),
    SUM(net_rx * n) / SUM(CASE WHEN net_rx IS NOT NULL THEN n END),
    SUM(net_tx * n) / SUM(CASE WHEN net_tx IS NOT NULL THEN n END),
    SUM(disk_read * n) / SUM(CASE WHEN disk_read IS NOT NULL THEN n END),
    SUM(disk_write * n) / SUM(CASE WHEN disk_write IS NOT NULL THEN n END),
    SUM(temp * n) / SUM(CASE WHEN temp IS NOT NULL THEN n END),
    SUM(gpu * n) / SUM(CASE WHEN gpu IS NOT NULL THEN n END)
FROM metric_points WHERE res = 900
GROUP BY host_id, (ts / 3600) * 3600
ON CONFLICT (host_id, res, ts) DO NOTHING;
