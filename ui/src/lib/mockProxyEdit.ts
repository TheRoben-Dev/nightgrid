/**
 * Browser-only stand-in for the reverse proxy route editor (M10v): a Caddy
 * on the host and a Traefik container with a file provider, edited in memory.
 */
import type {
  CaddyEdit,
  CaddyPreview_Serialize as CaddyPreview,
  CaddySite,
  DiffLine_Serialize as DiffLine,
  EditTargets_Serialize as EditTargets,
  SaveOutcome,
  TraefikRoute_Serialize as TraefikRoute,
  TraefikTarget_Serialize as TraefikTarget,
} from "./bindings";

let caddyText = `{
\temail admin@example.com
}

media.lan {
\treverse_proxy localhost:8096
}

files.example.com {
\treverse_proxy /api/* api:9000
\treverse_proxy web:80
}
`;
let traefik: TraefikRoute[] = [
  { name: "wiki", domains: ["wiki.example.com"], upstream: "http://wiki:3000", entrypoints: ["websecure"], tls: true, certResolver: "le" },
];
let version = 1;

const hash = (s: string) => `${s.length.toString(16)}-${version}`;

/** Top-level site blocks (the same rules as the backend, simplified). */
export function sites(text: string): CaddySite[] {
  const out: CaddySite[] = [];
  let depth = 0;
  let cur: { addresses: string[]; first: number; ups: string[]; simple: number; complex: boolean } | null = null;
  text.split("\n").forEach((raw, i) => {
    const line = raw.trim().startsWith("#") ? "" : raw.trim();
    if (!line) return;
    if (line === "}") {
      depth = Math.max(0, depth - 1);
      if (depth === 0 && cur) {
        out.push({ addresses: cur.addresses, firstLine: cur.first, lastLine: i, upstreams: cur.ups, simple: !cur.complex && cur.simple === 1 });
        cur = null;
      }
      return;
    }
    const opens = line === "{" || line.endsWith(" {");
    const head = line.replace(/\{$/, "").trim();
    if (depth === 0 && opens && head && !head.startsWith("(")) cur = { addresses: head.split(/[\s,]+/).filter(Boolean), first: i, ups: [], simple: 0, complex: false };
    else if (cur && head.startsWith("reverse_proxy")) {
      const args = head.split(/\s+/).slice(1);
      const matcher = (w: string) => w.startsWith("/") || w.startsWith("@") || w === "*";
      cur.ups.push(...args.filter((w) => !matcher(w)));
      if (depth === 1 && !opens && args.length && !args.some(matcher)) cur.simple++;
      else cur.complex = true;
    }
    if (opens) depth++;
  });
  return out;
}

function apply(text: string, e: CaddyEdit): string {
  const lines = text.replace(/\n$/, "").split("\n");
  const s = sites(text);
  const find = (a: string) => {
    const x = s.find((x) => x.addresses[0] === a);
    if (!x) throw new Error(`no site ${a} in the Caddyfile`);
    return x;
  };
  switch (e.op) {
    case "addSite":
      if (!e.domains.length) throw new Error("give at least one domain");
      if (!/^[\w.:/[\]-]+$/.test(e.upstream)) throw new Error(`"${e.upstream}" isn't an upstream (host:port or a URL)`);
      for (const d of e.domains) if (s.some((x) => x.addresses.includes(d))) throw new Error(`${d} already has a site`);
      return `${text.trimEnd()}\n\n${e.domains.join(", ")} {\n\treverse_proxy ${e.upstream}\n}\n`;
    case "setUpstream": {
      const x = find(e.site);
      if (!x.simple) throw new Error(`${e.site} has more than one reverse_proxy or a block: edit it as text`);
      for (let i = x.firstLine; i <= x.lastLine; i++) if (lines[i].trim().startsWith("reverse_proxy")) lines[i] = lines[i].replace(/reverse_proxy.*/, `reverse_proxy ${e.upstream}`);
      return `${lines.join("\n")}\n`;
    }
    case "removeSite": {
      const x = find(e.site);
      lines.splice(x.firstLine, x.lastLine - x.firstLine + 1);
      if (lines[x.firstLine - 1]?.trim() === "" && lines[x.firstLine]?.trim() === "") lines.splice(x.firstLine, 1);
      while (lines.length && !lines[lines.length - 1].trim()) lines.pop();
      return lines.length ? `${lines.join("\n")}\n` : "";
    }
    case "replace":
      return e.text;
  }
}

/** A plain LCS line diff (small files only). */
export function diffLines(a: string, b: string): DiffLine[] {
  const x = a.replace(/\n$/, "").split("\n");
  const y = b.replace(/\n$/, "").split("\n");
  const n = x.length;
  const m = y.length;
  const L = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) L[i][j] = x[i] === y[j] ? L[i + 1][j + 1] + 1 : Math.max(L[i + 1][j], L[i][j + 1]);
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && x[i] === y[j]) out.push({ tag: "same", old: ++i, new: ++j, text: x[i - 1] });
    else if (j < m && (i >= n || L[i][j + 1] >= L[i + 1][j])) out.push({ tag: "add", old: null, new: ++j, text: y[j - 1] });
    else out.push({ tag: "del", old: ++i, new: null, text: x[i - 1] });
  }
  return out;
}

const later = <T>(v: T, ms = 300) => new Promise<T>((r) => setTimeout(() => r(v), ms));

const traefikTarget = (): TraefikTarget => ({
  proxyName: "traefik",
  config: { directory: "/etc/traefik/dynamic", filename: null, entrypoints: ["web", "websecure"], resolvers: ["le"] },
  routes: traefik.map((r) => ({ ...r })),
  hash: `t-${version}`,
  blocked: null,
});

export const mockProxyEdit = {
  targets: (_hostId: number): Promise<EditTargets> =>
    later({ caddy: [{ proxyName: "host", path: "/etc/caddy/Caddyfile", text: caddyText, hash: hash(caddyText), sites: sites(caddyText) }], traefik: [traefikTarget()] }),
  preview: async (text: string, edit: CaddyEdit): Promise<CaddyPreview> => {
    const t = apply(text, edit);
    const lines = diffLines(text, t);
    return { text: t, diff: { path: "Caddyfile", lines, added: lines.filter((l) => l.tag === "add").length, removed: lines.filter((l) => l.tag === "del").length, binary: false, tooLarge: false, truncated: false } };
  },
  saveCaddy: async (_hostId: number, _proxy: string, text: string, baseHash: string): Promise<SaveOutcome> => {
    if (baseHash !== hash(caddyText)) throw new Error("the Caddyfile changed since it was opened: refresh and try again");
    if (/revers_proxy|\bbad\b/.test(text)) return later({ ok: false, message: "Caddy refused the new Caddyfile; nothing changed. Error: adapting config using caddyfile: unrecognized directive" }, 700);
    caddyText = text;
    version++;
    return later({ ok: true, message: "" }, 700);
  },
  saveTraefik: async (_hostId: number, _proxy: string, routes: TraefikRoute[], baseHash: string): Promise<TraefikTarget> => {
    if (baseHash !== `t-${version}`) throw new Error("Nightgrid's Traefik file changed since it was opened: refresh and try again");
    traefik = routes.map((r) => ({ ...r, upstream: r.upstream.includes("://") ? r.upstream : `http://${r.upstream}`, tls: r.tls || !!r.certResolver }));
    version++;
    return later(traefikTarget(), 500);
  },
};
