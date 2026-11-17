/** Browser-preview stand-in for DRIFT's read-only host checks (D-151). */
import type { Lookup_Serialize as Lookup, LookupResult_Serialize as LookupResult } from "./bindings";

const OUT: Partial<Record<Lookup["kind"], [string, string]>> = {
  overview: [
    "uptime; df -hP; free -m",
    " 20:14:02 up 12 days,  3:41,  1 user,  load average: 0.42, 0.37, 0.31\n\nFilesystem      Size  Used Avail Use% Mounted on\n/dev/sda2       234G  201G   21G  91% /\n\n               total        used        free\nMem:           15890        6120        1204",
  ],
  disk: ["df -hP; df -iP", "Filesystem      Size  Used Avail Use% Mounted on\n/dev/sda2       234G  201G   21G  91% /\n/dev/sdb1       3.6T  2.1T  1.5T  59% /srv/media"],
  failedUnits: ["systemctl --failed", "backup-nightly.service loaded failed failed Nightly backup"],
  errors: ["journalctl -p err -n 40", "2026-11-17T03:00:12+0100 server01 restic[2211]: Fatal: unable to open repository: connection refused"],
  containers: ["docker ps -a", "web\tnginx:1.27\tUp 3 days\ndb\tpostgres:16\tUp 3 days (healthy)\nworker\tapp/worker:2.1\tRestarting (1) 20 seconds ago"],
  dockerEvents: ["docker events --since 1h", "2026-11-17T19:58:01 container die worker\n2026-11-17T19:58:02 container restart worker"],
  kernelLog: ["journalctl -k -p warning", "(no output, exit code 0)"],
};

export function mockLookups(lookups: Lookup[], terms: string[]): Promise<LookupResult[]> {
  const all = [...lookups];
  if (terms.includes("worker")) all.unshift({ kind: "container", target: "worker" }, { kind: "containerLogs", target: "worker" });
  const res = all.slice(0, 8).map((l): LookupResult => {
    if (l.kind === "container") return { lookup: l, command: `docker inspect '${l.target}'`, ok: true, output: "name=/worker image=app/worker:2.1 state=restarting health=none restarts=14 policy=unless-stopped exit=1 oom=false", clipped: false };
    if (l.kind === "containerLogs") return { lookup: l, command: `docker logs --tail 50 '${l.target}'`, ok: true, output: "2026-11-17T19:58:00Z Error: connect ECONNREFUSED db:5432\n2026-11-17T19:58:00Z DATABASE_URL=postgres://app:••••@db/app", clipped: false };
    const o = OUT[l.kind];
    return o ? { lookup: l, command: o[0], ok: true, output: o[1], clipped: false } : { lookup: l, command: l.kind, ok: true, output: "(no output, exit code 0)", clipped: false };
  });
  return new Promise((r) => setTimeout(() => r(res), 400));
}
