/**
 * Compose review (M10p): exact checks on a docker-compose file that a small
 * model tends to miss or invent. Uses a small YAML reader for the subset
 * compose files use (block maps and lists, flow lists, scalars); anchors and
 * merge keys are read as plain values.
 */
import type { Finding } from "./command";

type Y = string | Y[] | { [k: string]: Y } | null;

interface Line {
  indent: number;
  text: string;
}

function unquote(s: string): string {
  const t = s.trim();
  if ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith("'") && t.endsWith("'"))) return t.slice(1, -1);
  return t;
}

/** Strips a trailing comment that isn't inside quotes. */
function uncomment(s: string): string {
  let q: string | null = null;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (q) {
      if (c === q) q = null;
    } else if (c === '"' || c === "'") q = c;
    else if (c === "#" && (i === 0 || /\s/.test(s[i - 1]))) return s.slice(0, i);
  }
  return s;
}

function scalar(raw: string): Y {
  const t = raw.trim();
  if (t === "" || t === "~" || t === "null") return null;
  if (t.startsWith("[") && t.endsWith("]")) {
    const inner = t.slice(1, -1).trim();
    return inner ? inner.split(",").map((x) => unquote(x)) : [];
  }
  if (t.startsWith("{") && t.endsWith("}")) {
    const o: Record<string, Y> = {};
    for (const part of t.slice(1, -1).split(",")) {
      const i = part.indexOf(":");
      if (i > 0) o[unquote(part.slice(0, i))] = unquote(part.slice(i + 1));
    }
    return o;
  }
  return unquote(t);
}

/** Index of the `key: value` colon, or -1 (ignores colons in quotes and in `8080:80`). */
function keyColon(s: string): number {
  let q: string | null = null;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (q) {
      if (c === q) q = null;
    } else if (c === '"' || c === "'") {
      if (i === 0) q = c;
    } else if (c === ":" && (i === s.length - 1 || s[i + 1] === " ")) return i;
  }
  return -1;
}

/** Reads the compose subset of YAML. Returns null when it can't make sense of it. */
export function parseYaml(text: string): Y {
  const lines: Line[] = [];
  let block: number | null = null;
  for (const raw of text.replace(/\t/g, "  ").split("\n")) {
    const indent = raw.length - raw.trimStart().length;
    if (block !== null) {
      if (raw.trim() === "" || indent > block) continue;
      block = null;
    }
    const t = uncomment(raw).trimEnd();
    if (!t.trim() || t.trim() === "---") continue;
    lines.push({ indent, text: t.trim() });
    if (/[:-]\s*[|>][-+]?\d*$/.test(t)) block = indent;
  }
  let i = 0;
  function parse(indent: number): Y {
    if (i >= lines.length) return null;
    if (lines[i].text.startsWith("- ") || lines[i].text === "-") {
      const arr: Y[] = [];
      const ind = lines[i].indent;
      while (i < lines.length && lines[i].indent === ind && (lines[i].text.startsWith("- ") || lines[i].text === "-")) {
        const rest = lines[i].text.slice(1).trim();
        if (!rest) {
          i++;
          arr.push(i < lines.length && lines[i].indent > ind ? parse(lines[i].indent) : null);
          continue;
        }
        const c = keyColon(rest);
        if (c > 0) {
          // A map inside a list item: "- key: value" and keys aligned below.
          const inner = ind + 2;
          lines[i] = { indent: inner, text: rest };
          arr.push(parse(inner));
        } else {
          i++;
          arr.push(scalar(rest));
        }
      }
      return arr;
    }
    const obj: Record<string, Y> = {};
    const ind = lines[i].indent;
    if (ind < indent) return null;
    while (i < lines.length && lines[i].indent === ind && !lines[i].text.startsWith("- ")) {
      const t = lines[i].text;
      const c = keyColon(t);
      if (c <= 0) {
        i++;
        continue;
      }
      const key = unquote(t.slice(0, c));
      const val = t.slice(c + 1).trim();
      i++;
      if (val && !/^[|>][-+]?\d*$/.test(val)) obj[key] = scalar(val);
      else if (/^[|>]/.test(val)) obj[key] = "";
      else if (i < lines.length && (lines[i].indent > ind || (lines[i].indent === ind && lines[i].text.startsWith("- ")))) obj[key] = parse(lines[i].indent);
      else obj[key] = null;
    }
    return obj;
  }
  try {
    return parse(0);
  } catch {
    return null;
  }
}

const isMap = (y: Y): y is Record<string, Y> => !!y && typeof y === "object" && !Array.isArray(y);
const str = (y: Y): string => (typeof y === "string" ? y : "");

export function looksLikeCompose(text: string): boolean {
  return /^services:\s*$/m.test(text) && /^\s+(image|build):/m.test(text);
}

const SECRET_NAME = /(pass(word|wd)?|secret|token|api[_-]?key|access[_-]?key|private[_-]?key|credential|auth)/i;
const SENSITIVE_PORT: Record<string, string> = {
  "2375": "the Docker API without TLS (full control of the host)",
  "2376": "the Docker API",
  "3306": "MySQL/MariaDB",
  "5432": "PostgreSQL",
  "6379": "Redis (no password by default)",
  "27017": "MongoDB",
  "9200": "Elasticsearch",
  "11211": "Memcached",
  "5984": "CouchDB",
  "8086": "InfluxDB",
};

/** `ip:host:container`, `host:container` or long syntax → host ip and published port. */
function portBind(p: Y): { ip: string | null; host: string; spec: string } | null {
  if (isMap(p)) {
    const pub = str(p.published ?? null) || (typeof p.published === "number" ? String(p.published) : "");
    if (!pub) return null;
    return { ip: str(p.host_ip ?? null) || null, host: pub, spec: `${p.host_ip ? `${str(p.host_ip)}:` : ""}${pub}:${str(p.target ?? null)}` };
  }
  const s = str(p).replace(/\/(tcp|udp)$/, "");
  if (!s.includes(":")) return null;
  const v6 = /^\[([^\]]+)\]:(.+)$/.exec(s);
  if (v6) {
    const [host] = v6[2].split(":");
    return { ip: v6[1], host, spec: s };
  }
  const parts = s.split(":");
  if (parts.length >= 3) return { ip: parts[0] || null, host: parts[1], spec: s };
  return { ip: null, host: parts[0], spec: s };
}

const LOOPBACK = /^(127\.\d+\.\d+\.\d+|::1|localhost)$/;

export interface ComposeReview {
  services: string[];
  findings: (Finding & { service?: string })[];
  /** Set when the file couldn't be read. */
  error?: string;
}

/** Checks each service: restart policy, image tags, published ports, inline secrets, privileges. */
export function reviewCompose(text: string): ComposeReview {
  const doc = parseYaml(text);
  if (!isMap(doc) || !isMap(doc.services ?? null)) return { services: [], findings: [], error: "No services: section found (or the YAML couldn't be read)." };
  const services = doc.services as Record<string, Y>;
  const out: ComposeReview["findings"] = [];
  const add = (service: string | undefined, level: Finding["level"], what: string, why: string) => out.push({ service, level, what, why });
  if ("version" in doc) add(undefined, "info", "version:", "Obsolete: Compose v2 ignores it and prints a warning. Safe to delete.");
  const noHealth: string[] = [];
  for (const [name, svc] of Object.entries(services)) {
    if (!isMap(svc)) continue;
    const image = str(svc.image ?? null);
    const deploy = isMap(svc.deploy ?? null) ? (svc.deploy as Record<string, Y>) : null;
    if (!svc.restart && !(deploy && deploy.restart_policy)) add(name, "caution", "no restart policy", "Won't start again after a crash or a reboot. Add `restart: unless-stopped`.");
    else if (str(svc.restart ?? null) === "no") add(name, "info", "restart: no", "Stays down after a crash or reboot (fine for one-off jobs).");
    if (image) {
      const ref = image.split("@")[0];
      const last = ref.split("/").pop() ?? ref;
      if (!image.startsWith("$") && !image.includes("@") && (!last.includes(":") || last.endsWith(":latest"))) add(name, "caution", last.includes(":") ? `${image}` : `${image} (no tag)`, "Means \"latest\": a pull can bring a new major version without warning. Pin a version tag.");
    } else if (!svc.build) add(name, "caution", "no image", "The service has neither image nor build.");
    const ports = Array.isArray(svc.ports) ? svc.ports : [];
    for (const p of ports) {
      const b = portBind(p);
      if (!b || (b.ip && LOOPBACK.test(b.ip))) continue;
      const known = SENSITIVE_PORT[b.host] ?? SENSITIVE_PORT[(str(p).split(":").pop() ?? "").replace(/\/.*/, "")];
      if (known && (b.host === "2375" || b.host === "2376")) add(name, "danger", b.spec, `Publishes ${known} on every network interface.`);
      else if (known) add(name, "caution", b.spec, `Publishes ${known} on every network interface. If only other containers use it, drop the port; otherwise bind it to 127.0.0.1 or a LAN address.`);
      else if (!b.ip || b.ip === "0.0.0.0" || b.ip === "::") add(name, "info", b.spec, "Listens on every interface (LAN, VPN and, if forwarded, the internet). Use 127.0.0.1: in front if only a reverse proxy on this host needs it.");
    }
    const env = svc.environment ?? null;
    const pairs: [string, string][] = Array.isArray(env)
      ? env.map((e) => {
          const s = str(e);
          const i = s.indexOf("=");
          return (i > 0 ? [s.slice(0, i), s.slice(i + 1)] : [s, ""]) as [string, string];
        })
      : isMap(env)
        ? Object.entries(env).map(([k, v]) => [k, str(v)] as [string, string])
        : [];
    for (const [k, v] of pairs) {
      if (SECRET_NAME.test(k) && !/_FILE$/i.test(k) && v && !v.includes("${") && !v.startsWith("$")) add(name, "caution", `${k}=…`, `A secret written into the compose file: anyone who can read it (or a git repo) sees it. Move it to an .env file next to it (${k}: $\{${k}}) or use Docker secrets.`);
    }
    if (svc.privileged === "true") add(name, "danger", "privileged: true", "Gives the container every device and kernel capability: escaping to the host is easy. Add only the devices or cap_add it really needs.");
    if (str(svc.network_mode ?? null) === "host") add(name, "info", "network_mode: host", "Uses the host's network directly: every port it opens is open on the host, and ports: is ignored.");
    if (str(svc.pid ?? null) === "host") add(name, "caution", "pid: host", "Sees and can signal every process on the host.");
    const caps = Array.isArray(svc.cap_add) ? svc.cap_add.map(str) : [];
    if (caps.some((c) => /^(ALL|SYS_ADMIN|CAP_SYS_ADMIN)$/i.test(c))) add(name, "caution", `cap_add: ${caps.filter((c) => /ALL|SYS_ADMIN/i.test(c)).join(", ")}`, "Nearly root on the host; most images that ask for it need something narrower.");
    const vols = Array.isArray(svc.volumes) ? svc.volumes : [];
    for (const v of vols) {
      const src = isMap(v) ? str(v.source ?? null) : str(v).split(":")[0];
      if (/^\/(var\/)?run\/docker\.sock$/.test(src)) add(name, "caution", "docker.sock", "Can control Docker, which equals root on the host (read-only :ro doesn't change that). Use a socket proxy if it only needs to read.");
      else if (src === "/") add(name, "danger", "/ mounted", "The host's whole filesystem is inside the container.");
      else if (/^\/(etc|root|boot)\/?$/.test(src)) add(name, "caution", `${src} mounted`, "A sensitive host folder is mounted into the container.");
    }
    if (!svc.healthcheck) noHealth.push(name);
  }
  if (noHealth.length && noHealth.length === Object.keys(services).length && noHealth.length > 1) add(undefined, "info", "no healthchecks", "No service has a healthcheck, so `depends_on` can't wait for one to be ready and Docker can't tell a hung service from a healthy one.");
  const rank = { danger: 0, caution: 1, info: 2 };
  out.sort((a, b) => rank[a.level] - rank[b.level]);
  return { services: Object.keys(services), findings: out };
}
