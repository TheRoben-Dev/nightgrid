/**
 * Browser-only stand-in for the firewall manager (M10v): ufw on host 1,
 * firewalld elsewhere, changes applied in memory with the same pending /
 * keep / undo flow.
 */
import type {
  ChangeCheck_Serialize as ChangeCheck,
  FirewallReport_Serialize as FirewallReport,
  FirewallView_Serialize as FirewallView,
  FwChange_Serialize as FwChange,
  PendingChange,
} from "./bindings";

const ufw = (): FirewallReport => ({
  backend: "ufw",
  active: true,
  ufw: {
    defaultIn: "deny",
    defaultOut: "allow",
    rules: [
      { spec: "allow 22/tcp", action: "allow", comment: null },
      { spec: "allow from 192.168.1.0/24 to any port 80,443 proto tcp comment 'lan web'", action: "allow", comment: "lan web" },
      { spec: "limit 2222/tcp comment 'alt ssh'", action: "limit", comment: "alt ssh" },
      { spec: "deny 23", action: "deny", comment: null },
    ],
  },
  zones: [],
  raw: "table inet filter {\n\tchain input {\n\t\ttype filter hook input priority filter; policy accept;\n\t}\n}",
  rollbackArmed: false,
  windows: null,
});
const fwd = (): FirewallReport => ({
  backend: "firewalld",
  active: true,
  ufw: null,
  zones: [
    { name: "public", active: true, default: true, target: "default", interfaces: ["eth0"], sources: [], services: ["dhcpv6-client", "ssh", "cockpit"], ports: ["8080/tcp"], richRules: ['rule family="ipv4" source address="192.168.1.20" accept'] },
    { name: "trusted", active: true, default: false, target: "ACCEPT", interfaces: [], sources: ["10.0.0.0/8"], services: [], ports: [], richRules: [] },
  ],
  raw: "",
  rollbackArmed: false,
  windows: null,
});

const state: Record<number, { report: FirewallReport; before: FirewallReport | null; pending: PendingChange | null }> = {};
const get = (id: number) => (state[id] ??= { report: id === 1 ? ufw() : fwd(), before: null, pending: null });
const wait = (ms = 250) => new Promise((r) => setTimeout(r, ms));
const clone = <T>(x: T): T => JSON.parse(JSON.stringify(x));

function describe(c: FwChange): string {
  switch (c.op) {
    case "ufwAdd":
      return `${c.action} ${c.port}${c.proto === "any" ? "" : `/${c.proto}`}${c.from ? ` from ${c.from}` : ""}`;
    case "ufwDelete":
      return `delete rule \`${c.spec}\``;
    case "ufwEnable":
      return c.enabled ? "turn ufw on" : "turn ufw off";
    case "ufwDefault":
      return `set the incoming default to ${c.policy}`;
    case "firewalldService":
      return `${c.add ? "add" : "remove"} service ${c.service} ${c.add ? "to" : "from"} zone ${c.zone}`;
    case "firewalldPort":
      return `${c.add ? "open" : "close"} port ${c.port}/${c.proto} in zone ${c.zone}`;
    case "firewalldSource":
      return `${c.add ? "accept" : "stop accepting"} ${c.source}${c.port ? ` on port ${c.port}` : ""} in zone ${c.zone}`;
    default:
      return c.op;
  }
}
function risk(r: FirewallReport, c: FwChange): string | null {
  const msg = (w: string) => `${w}: Nightgrid connects on port 22 and could be locked out.`;
  const sshAllowed = (without?: string) => r.ufw?.rules.some((x) => x.spec !== without && x.action !== "deny" && x.action !== "reject" && /\b22\b|OpenSSH/.test(x.spec));
  if (c.op === "ufwDelete" && /\b22\b/.test(c.spec) && !sshAllowed(c.spec)) return msg("This is the rule that lets SSH in");
  if (c.op === "ufwAdd" && (c.action === "deny" || c.action === "reject") && c.port === "22") return msg("This blocks the SSH port");
  if (c.op === "ufwEnable" && c.enabled && !sshAllowed()) return msg("No ufw rule allows SSH");
  if (c.op === "firewalldService" && !c.add && c.service === "ssh") return msg("This removes the ssh service");
  return null;
}
function applyTo(r: FirewallReport, c: FwChange) {
  const z = "zone" in c ? r.zones.find((x) => x.name === c.zone) : undefined;
  if (c.op === "ufwAdd" && r.ufw) r.ufw.rules.push({ spec: `${c.action} ${c.proto === "any" ? "" : `proto ${c.proto} `}from ${c.from || "any"} to any port ${c.port}${c.comment ? ` comment '${c.comment}'` : ""}`, action: c.action, comment: c.comment });
  else if (c.op === "ufwDelete" && r.ufw) r.ufw.rules = r.ufw.rules.filter((x) => x.spec !== c.spec);
  else if (c.op === "ufwEnable") r.active = c.enabled;
  else if (c.op === "ufwDefault" && r.ufw) r.ufw.defaultIn = c.policy;
  else if (c.op === "firewalldService" && z) z.services = c.add ? [...z.services, c.service] : z.services.filter((s) => s !== c.service);
  else if (c.op === "firewalldPort" && z) z.ports = c.add ? [...z.ports, `${c.port}/${c.proto}`] : z.ports.filter((p) => p !== `${c.port}/${c.proto}`);
  else if (c.op === "firewalldSource" && z) {
    const rule = `rule family="ipv4" source address="${c.source}"${c.port ? ` port port="${c.port}" protocol="${c.proto}"` : ""} accept`;
    z.richRules = c.add ? [...z.richRules, rule] : z.richRules.filter((x) => x !== rule);
  }
}

export const mockFirewall = {
  async read(id: number): Promise<FirewallView> {
    await wait();
    const s = get(id);
    if (s.pending && s.pending.deadline < Date.now() / 1000) {
      if (s.before) s.report = s.before;
      s.pending = null;
    }
    return { report: clone(s.report), pending: s.pending, sshPort: 22 };
  },
  async check(id: number, change: FwChange): Promise<ChangeCheck> {
    return { summary: describe(change), risk: risk(get(id).report, change) };
  },
  async apply(id: number, change: FwChange, secs: number, typed: string | null): Promise<PendingChange> {
    await wait(400);
    const s = get(id);
    if (risk(s.report, change) && !typed?.trim()) throw new Error("type the host name to confirm a change that could lock you out");
    if (s.pending) throw new Error("keep or undo the last change first");
    s.before = clone(s.report);
    applyTo(s.report, change);
    s.pending = { token: String(Math.floor(Date.now() / 1000)), deadline: Math.floor(Date.now() / 1000) + Math.min(900, Math.max(30, secs)), change: describe(change) };
    return s.pending;
  },
  async test(id: number): Promise<number> {
    await wait(600);
    if (get(id).report.backend === "ufw" && get(id).report.ufw?.rules.every((r) => !/\b22\b/.test(r.spec)) && get(id).report.active) throw new Error("a new connection timed out");
    return 184;
  },
  async finish(id: number, token: string, keep: boolean): Promise<null> {
    await wait();
    const s = get(id);
    if (!s.pending || s.pending.token !== token) throw new Error("no change is waiting on this host (it may have rolled back already)");
    if (!keep && s.before) s.report = s.before;
    s.pending = null;
    s.before = null;
    return null;
  },
};
