/** Browser-only stand-in for blueprints (0.10.84). */
import type { BlueprintSummary_Serialize as BlueprintSummary, BpInclude_Deserialize as BpInclude, BpStep, BpStepResult } from "./bindings";

const sample = (host: string, inc?: BpInclude): BlueprintSummary => ({
  host,
  madeAt: Math.floor(Date.now() / 1000) - 3600,
  os: "Debian GNU/Linux 12 (bookworm)",
  stacks: [
    { name: "media", dir: "/srv/media", files: ["/srv/media/compose.yaml"], env: ["TZ", "PUID", "DB_PASSWORD"] },
    { name: "photos", dir: "/srv/photos", files: ["/srv/photos/docker-compose.yml", "/srv/photos/.env"], env: [] },
  ],
  crontabs: inc?.cron === false ? [] : [{ user: "", lines: 3 }],
  timers: inc?.timers === false ? [] : ["scrub", "restic-check"],
  firewall: inc?.firewall === false ? null : "ufw",
  firewallRules: inc?.firewall === false ? 0 : 5,
  proxies: inc?.proxy === false ? [] : ["Caddy (host): /etc/caddy/Caddyfile"],
  notes: [],
});

let opened = false;

export const mockBlueprint = {
  async capture(hostId: number, include: BpInclude): Promise<BlueprintSummary> {
    await new Promise((r) => setTimeout(r, 400));
    return sample(`host ${hostId}`, include);
  },
  async save(): Promise<string | null> {
    return "/Users/you/Documents/server01-2026-11-17.ngblueprint";
  },
  async open(passphrase: string): Promise<BlueprintSummary | null> {
    if (passphrase.length < 8) throw new Error("wrong passphrase, or the file isn't a blueprint");
    opened = true;
    return sample("server01");
  },
  async plan(): Promise<BpStep[]> {
    if (!opened) throw new Error("open a blueprint first");
    const s = (id: string, kind: BpStep["kind"], title: string, detail: string, state: BpStep["state"]): BpStep => ({ id, kind, title, detail, state, defaultOn: state === "new" });
    return [
      s("stack:media", "stackFiles", "Stack media: write files", "2 files in /srv/media; 3 .env values into the vault", "new"),
      s("start:media", "stackStart", "Stack media: start", "docker compose up -d (pulls the images)", "new"),
      s("stack:photos", "stackFiles", "Stack photos: write files", "2 files in /srv/photos; overwrites /srv/photos/.env", "differs"),
      s("start:photos", "stackStart", "Stack photos: start", "a stack with this name already runs there", "same"),
      s("cron:", "cron", "Crontab: the login user's crontab", "adds 2 lines; existing lines stay", "new"),
      s("timer:scrub", "timer", "Timer scrub", "Sun 03:00 · zpool scrub tank", "new"),
      s("timer:restic-check", "timer", "Timer restic-check", "Mon 05:00 · restic check", "same"),
      s("fw:allow 22/tcp", "firewall", "ufw allow 22/tcp", "already there", "same"),
      s("fw:allow 443/tcp", "firewall", "ufw allow 443/tcp", "adds an allow rule", "new"),
      s("fw:deny 23", "firewall", "ufw deny 23", "deny and reject rules are added by hand in System → FIREWALL, behind its rollback", "manual"),
      s("caddy:host", "caddy", "Caddyfile (host)", "no Caddy there yet: install Caddy first, then plan again", "manual"),
    ];
  },
  async apply(_hostId: number, steps: string[]): Promise<BpStepResult[]> {
    await new Promise((r) => setTimeout(r, 600));
    return steps.map((id) => ({ id, ok: !id.startsWith("start:photos"), message: id.startsWith("stack:") ? "2 files written" : "done" }));
  },
  async close(): Promise<null> {
    opened = false;
    return null;
  },
};
