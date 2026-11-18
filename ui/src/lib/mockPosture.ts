/** Browser-only stand-in for Security → POSTURE (1.1). */
import type { PostureCheck_Serialize as PostureCheck, PostureHost_Serialize as PostureHost, PostureRun_Serialize as PostureRun } from "./bindings";
import { mockBackend } from "./mock";

const runs = new Map<number, PostureRun[]>();
const now = () => Math.floor(Date.now() / 1000);

function make(hostId: number, i: number): PostureRun {
  const bad = i % 3;
  const checks: PostureCheck[] = [
    { id: "ssh-root", title: "Root can't log in over SSH", status: "pass", detail: "PermitRootLogin no.", fix: null, weight: 20, article: "basics/sshd-hardening" },
    { id: "ssh-password", title: "SSH needs keys, not passwords", status: bad === 1 ? "fail" : "pass", detail: bad === 1 ? "Password logins are allowed and SSH listens on every interface." : "Password and keyboard-interactive logins are off.", fix: bad === 1 ? "Set `PasswordAuthentication no` and `KbdInteractiveAuthentication no` under `/etc/ssh/sshd_config.d/`, then `sudo sshd -t && sudo systemctl reload ssh`." : null, weight: 15, article: "basics/sshd-hardening" },
    { id: "firewall", title: "A firewall is on", status: bad ? "warn" : "pass", detail: bad ? "No firewall is active." : "ufw is active.", fix: bad ? "`sudo ufw allow OpenSSH && sudo ufw enable`" : null, weight: 15, article: "howto/firewall" },
    { id: "exposed-services", title: "Databases aren't open to the network", status: bad === 2 ? "fail" : "pass", detail: bad === 2 ? "postgres 5432, redis 6379 listen on every interface." : "No database, cache or Docker API port listens on every interface.", fix: bad === 2 ? "Bind them to `127.0.0.1:` in compose." : null, weight: 20, article: "guides/security-posture" },
    { id: "intrusion", title: "Brute-force protection", status: "pass", detail: "fail2ban or CrowdSec is running.", fix: null, weight: 5, article: "apps/fail2ban-crowdsec" },
    { id: "auto-updates", title: "Security updates install themselves", status: bad ? "warn" : "pass", detail: bad ? "No automatic security updates found." : "unattended-upgrades is set up.", fix: null, weight: 10, article: "apps/unattended-upgrades" },
    { id: "reboot", title: "No reboot pending", status: "pass", detail: "Nothing is waiting for a reboot.", fix: null, weight: 5, article: "guides/security-posture" },
    { id: "uid0", title: "Only root has uid 0", status: "pass", detail: "No other account has root's uid.", fix: null, weight: 20, article: "howto/access-audit" },
    { id: "images-fresh", title: "Container images are under six months old", status: bad === 2 ? "warn" : "pass", detail: bad === 2 ? "2 of 9 older than 180 days, oldest postgres:13 (402 days)" : "9 images built in the last 180 days", fix: null, weight: 10, article: "guides/security-posture" },
    { id: "security-updates", title: "No security updates waiting", status: bad === 1 ? "fail" : "pass", detail: bad === 1 ? "6 security updates waiting" : "no security updates waiting", fix: null, weight: 15, article: "howto/fleet-updates" },
  ];
  const lost = checks.reduce((a, c) => a + (c.status === "fail" ? c.weight : c.status === "warn" ? Math.floor(c.weight / 2) : 0), 0);
  const score = Math.max(0, 100 - lost);
  const grade = score >= 90 ? "A" : score >= 75 ? "B" : score >= 60 ? "C" : score >= 40 ? "D" : "F";
  return { hostId, at: now(), score, grade, checks, openPorts: 2 + bad, error: null };
}

export const mockPosture = {
  async list(): Promise<PostureHost[]> {
    const hs = await mockBackend.hostsList();
    return hs
      .filter((h) => runs.has(h.id))
      .map((h) => {
        const r = runs.get(h.id)!;
        return { latest: r[0], previous: r[1]?.score ?? null, trend: r.map((x) => x.score ?? 0).reverse() };
      });
  },
  async history(hostId: number): Promise<PostureRun[]> {
    return runs.get(hostId) ?? [];
  },
  async check(hostIds: number[]): Promise<PostureRun[]> {
    const hs = await mockBackend.hostsList();
    return hostIds.map((id) => {
      const i = hs.findIndex((h) => h.id === id);
      const r = make(id, i);
      const prev = runs.get(id) ?? [{ ...r, score: Math.min(100, (r.score ?? 0) + 6) }];
      runs.set(id, [r, ...prev].slice(0, 30));
      return r;
    });
  },
};
