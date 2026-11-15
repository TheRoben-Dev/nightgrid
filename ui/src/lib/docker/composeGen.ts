/**
 * The deploy wizard's "from an image" form → compose.yaml text (and .env).
 * Env values never go into compose.yaml: they are referenced as ${NAME} and
 * written to the stack's .env (mode 600 on the host).
 */
import type { RemoteImage } from "../api";

export interface PortRow {
  host: string;
  container: string;
  /** "tcp" | "udp" */
  proto: string;
}
export interface VolumeRow {
  host: string;
  container: string;
}
export interface EnvRow {
  name: string;
  value: string;
}
export interface ServiceForm {
  name: string;
  image: string;
  restart: "unless-stopped" | "always" | "on-failure" | "no";
  ports: PortRow[];
  volumes: VolumeRow[];
  env: EnvRow[];
}

/** A compose project / service name from free text. */
export function slug(s: string): string {
  const v = s
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^[-_]+|[-_]+$/g, "")
    .slice(0, 40);
  return v || "app";
}

/** A sensible project name for an image: `ghcr.io/acme/web-app:2` → `web-app`. */
export function nameFromImage(image: string): string {
  const repo = image.split("@")[0].split("/").pop() ?? image;
  return slug(repo.split(":")[0]);
}

/** Pre-fills the form from what the image declares. */
export function formFromImage(info: RemoteImage, name = nameFromImage(info.image)): ServiceForm {
  return {
    name,
    image: info.image,
    restart: "unless-stopped",
    ports: info.ports.map((p) => {
      const [port, proto = "tcp"] = p.split("/");
      return { host: port, container: port, proto };
    }),
    volumes: info.volumes.map((v) => ({ host: `./${slug(v.split("/").filter(Boolean).pop() ?? "data")}`, container: v })),
    env: info.env.map((n) => ({ name: n, value: "" })),
  };
}

export const envNameOk = (n: string) => /^[A-Za-z_][A-Za-z0-9_]*$/.test(n);

/** Quotes a YAML scalar when it needs it. */
export function yamlStr(s: string): string {
  if (
    s === "" ||
    /^\s|\s$|: |:$| #|["'\\]/.test(s) ||
    /^[-?:,[\]{}#&*!|>%@`.\d]/.test(s) ||
    /^(true|false|yes|no|null|~|on|off|y|n)$/i.test(s)
  ) {
    return `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
  }
  return s;
}

/** Problems that would make the file wrong, in words. */
export function formProblems(f: ServiceForm): string[] {
  const out: string[] = [];
  if (!f.image.trim()) out.push("Pick an image.");
  if (!/^[a-z0-9][a-z0-9_-]*$/.test(f.name)) out.push("The name takes lowercase letters, digits, - and _.");
  for (const p of f.ports) {
    if (!p.container.trim()) continue;
    const ok = (v: string) => /^\d{1,5}$/.test(v) && +v > 0 && +v < 65536;
    if (!ok(p.container) || (p.host.trim() && !ok(p.host))) out.push(`Port ${p.host || "?"}:${p.container} isn't a valid port number.`);
  }
  const hostPorts = f.ports.filter((p) => p.host.trim() && p.container.trim()).map((p) => `${p.host}/${p.proto}`);
  const dup = hostPorts.find((p, i) => hostPorts.indexOf(p) !== i);
  if (dup) out.push(`Host port ${dup} is used twice.`);
  for (const v of f.volumes) if (v.container.trim() && !v.container.startsWith("/")) out.push(`Container path ${v.container} must start with /.`);
  for (const e of f.env) if (e.name.trim() && !envNameOk(e.name.trim())) out.push(`${e.name} isn't a valid variable name.`);
  return out;
}

export function composeYaml(f: ServiceForm): string {
  const svc = slug(f.name);
  const lines = ["services:", `  ${svc}:`, `    image: ${yamlStr(f.image.trim())}`, `    container_name: ${svc}`, `    restart: ${f.restart}`];
  const ports = f.ports.filter((p) => p.container.trim());
  if (ports.length) {
    lines.push("    ports:");
    for (const p of ports) {
      const spec = `${p.host.trim() ? `${p.host.trim()}:` : ""}${p.container.trim()}${p.proto === "udp" ? "/udp" : ""}`;
      lines.push(`      - ${yamlStr(spec)}`);
    }
  }
  const vols = f.volumes.filter((v) => v.container.trim() && v.host.trim());
  if (vols.length) {
    lines.push("    volumes:");
    for (const v of vols) lines.push(`      - ${yamlStr(`${v.host.trim()}:${v.container.trim()}`)}`);
  }
  const env = f.env.filter((e) => envNameOk(e.name.trim()));
  if (env.length) {
    lines.push("    environment:");
    for (const e of env) lines.push(`      ${e.name.trim()}: ${yamlStr("${" + e.name.trim() + "}")}`);
  }
  return lines.join("\n") + "\n";
}

/** .env text for the form's variables (null when there are none). */
export function envText(f: ServiceForm): string | null {
  const env = f.env.filter((e) => envNameOk(e.name.trim()));
  if (!env.length) return null;
  // Single quotes are literal in compose's .env; a value with a ' in it is
  // double-quoted with \ and " escaped.
  const q = (v: string) =>
    !/[\s#"'$`\\]/.test(v) ? v : !v.includes("'") ? `'${v}'` : `"${v.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
  return env.map((e) => `${e.name.trim()}=${q(e.value)}`).join("\n") + "\n";
}
