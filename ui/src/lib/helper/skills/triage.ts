/**
 * Alert triage (M10p): for each kind of alert, the usual causes and the
 * first things to check, so DRIFT starts from known ground. The alert's own
 * details and recent error lines come with it.
 */
import type { AlertKind } from "../../api";

export interface Triage {
  causes: string[];
  checks: string[];
  /** Where in Nightgrid to look. */
  where: string;
}

export const TRIAGE: Record<AlertKind, Triage> = {
  hostOffline: {
    causes: ["Powered off, asleep or rebooting", "Network change: Wi-Fi, VPN or tailnet down, or a new IP address", "SSH stopped or a firewall now blocks port 22"],
    checks: ["`ping <address>` from this computer (and from another host on its LAN)", "Wake it (System → WAKE) if it sleeps", "Check the tailnet fallback address and whether the VPN is up", "If it answers ping but not SSH: open its console and `systemctl status ssh`"],
    where: "Overview → the host's tile, or System → POWER",
  },
  diskFull: {
    causes: ["Logs growing (journald, container json logs, app logs)", "Old Docker images, build cache and stopped containers", "Backups or downloads landing on the wrong disk"],
    checks: ["`df -h` and `df -i` (inodes)", "`sudo du -xh --max-depth=1 / | sort -h | tail`", "`docker system df` then prune what's unused", "`journalctl --disk-usage` and `sudo journalctl --vacuum-size=500M`"],
    where: "Fleet → DISKS (growth and forecast)",
  },
  containerDown: {
    causes: ["The app crashed (look at the exit code: 137 = killed, often out of memory; 1 = app error)", "A failing healthcheck or a dependency (database) not ready", "Stopped by hand, or a deploy that didn't come back"],
    checks: ["`docker logs --tail 100 <container>`", "`docker inspect -f '{{.State.ExitCode}} {{.State.OOMKilled}} {{.State.Error}}' <container>`", "Does the compose file have `restart: unless-stopped`?"],
    where: "Docker → the container's logs",
  },
  serviceDown: {
    causes: ["The app behind it is down or restarting", "Reverse proxy misrouted, or the certificate is invalid", "DNS or firewall change; the check runs from this computer, so a VPN being down looks the same"],
    checks: ["`curl -vk <url>` from this computer and from the host itself", "Is its container running and healthy?", "Proxy logs (Caddy/Traefik/nginx) for 502s"],
    where: "Services → the service's row",
  },
  certExpiring: {
    causes: ["Automatic renewal is failing (ACME challenge blocked, DNS API token expired)", "The certificate is managed by hand", "The proxy renewed but wasn't reloaded"],
    checks: ["`sudo certbot renew --dry-run` (or the proxy's logs for ACME errors)", "Is port 80 reachable for HTTP challenges, or is the DNS token still valid?", "Reload the proxy after renewing"],
    where: "Services → EXPIRY",
  },
  smartWarning: {
    causes: ["Reallocated or pending sectors: the disk is wearing out", "UDMA CRC errors: usually the cable or port, not the disk", "Running too hot"],
    checks: ["`sudo smartctl -a /dev/<disk>` and compare with last time", "Make sure the backups are current before anything else", "For CRC errors reseat or swap the cable"],
    where: "Fleet → DISKS",
  },
  backupFailed: {
    causes: ["The repository is locked by an interrupted run", "Wrong password or key file, or the target isn't mounted/reachable", "Out of space on the target"],
    checks: ["`journalctl -u <backup unit> -n 50`", "restic: `restic unlock` after making sure no run is active; borg: `borg break-lock`", "`df -h` on the target"],
    where: "Fleet → BACKUPS",
  },
  backupStale: {
    causes: ["The timer was disabled or never enabled", "The host was asleep or off at the scheduled time (no Persistent=true)", "A run hangs and blocks the next ones"],
    checks: ["`systemctl list-timers --all | grep <unit>`", "`systemctl status <unit>` for a run that never finished", "Add Persistent=true to the timer so missed runs catch up"],
    where: "Fleet → BACKUPS",
  },
  logBurst: {
    causes: ["One fault repeating (a crash loop, a dependency down, a full disk)", "A noisy client or scanner hitting an endpoint"],
    checks: ["Read the newest error lines (attached) and look for the first one", "Is the service restarting? `systemctl status` or `docker ps`", "Use the error reference below for the messages themselves"],
    where: "The log's view (journal or container logs)",
  },
  logNewError: {
    causes: ["Something changed: an update, a config edit, a new dependency version", "A first sign of a hardware or disk problem"],
    checks: ["What changed recently? System → TIMELINE and CONFIG drift", "Read the attached lines and the error reference below"],
    where: "System → TIMELINE",
  },
  expirySoon: {
    causes: ["A domain, API token or licence is near its end date"],
    checks: ["Renew it with the provider", "Update the date in the expiry watch afterwards"],
    where: "Services → EXPIRY",
  },
  diskFilling: {
    causes: ["Steady growth from logs, media, backups or databases at the current rate"],
    checks: ["`sudo du -xh --max-depth=2 <mount> | sort -h | tail` and compare in a day", "Set log rotation and retention, or plan more space"],
    where: "Fleet → DISKS (forecast)",
  },
  poolDegraded: {
    causes: ["A disk failed or dropped out of the pool", "A loose cable or controller reset"],
    checks: ["`zpool status -x` / `cat /proc/mdstat` / `btrfs device stats`", "Check backups, then replace the device (`zpool replace`)", "Avoid heavy writes and reboots until it's resilvered"],
    where: "Fleet → DISKS",
  },  upsOnBattery: {
    causes: ["A power cut or a tripped breaker", "The UPS input is unplugged or its fuse/outlet failed", "RB flag: the battery is worn out and won't hold a charge"],
    checks: ["`upsc <ups>` on the host it's plugged into: status, battery.charge, battery.runtime", "Is the shutdown group right, and can each host shut down without a sudo password?", "For RB: run the UPS self-test and plan a battery swap"],
    where: "Fleet → UPS",
  },
};
