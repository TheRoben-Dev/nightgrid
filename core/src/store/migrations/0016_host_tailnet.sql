-- 0016 (M10j): a per-host tailnet fallback address, tried when the primary
-- address doesn't answer (home LAN name vs. 100.x / MagicDNS name away), and
-- the per-host opt-in for DRIFT reading the host's recent shell history.

ALTER TABLE hosts ADD COLUMN tailnet_address TEXT;
ALTER TABLE hosts ADD COLUMN drift_history INTEGER NOT NULL DEFAULT 0;
