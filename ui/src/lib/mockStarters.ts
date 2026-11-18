/** Browser-only stand-in for starter blueprints and runbooks (D-175). The real set lives in starters/ and is read by the app. */
import type { BlueprintSummary_Serialize as BlueprintSummary, StarterCatalog_Serialize as StarterCatalog, StarterTarget, StarterValue } from "./bindings";

const catalog: StarterCatalog = {
  format: 1,
  blueprints: [
    {
      id: "media-server",
      title: "Media server",
      summary: "Jellyfin to watch your movies and shows anywhere at home, with Sonarr, Radarr and Prowlarr to keep the library organised.",
      category: "media",
      purpose: "Turns a host into your own streaming service. Jellyfin scans a folder of movies and shows and plays them on TVs, phones and browsers.\n\nOnly Jellyfin is opened on your network.",
      library: ["apps/jellyfin", "apps/arr-stack"],
      stacks: [
        {
          name: "media",
          compose: "services:\n  jellyfin:\n    image: jellyfin/jellyfin:latest\n    ports:\n      - \"8096:8096\"\n    environment:\n      TZ: ${TZ}\n    volumes:\n      - ${MEDIA_DIR}:/media\n",
          files: [],
          env: [
            { name: "TZ", label: "Time zone", default: "Etc/UTC", help: "Such as Europe/Stockholm.", secret: false, generate: false, path: false },
            { name: "MEDIA_DIR", label: "Folder holding your media", default: "/srv/media", help: "", secret: false, generate: false, path: true },
          ],
          ports: [
            { port: "8096/tcp", what: "Jellyfin", local: false },
            { port: "8989/tcp", what: "Sonarr", local: true },
          ],
        },
      ],
      firewall: { allow: ["8096/tcp"], limit: [], denyIncoming: false, enable: false },
      timers: [],
      manual: [{ title: "Finish Jellyfin's first-run wizard", detail: "Open http://<host>:8096 and create the admin user." }],
    },
    {
      id: "dns-filter",
      title: "Network-wide ad blocking",
      summary: "Pi-hole answers DNS for your whole network and blocks ads and trackers for every device.",
      category: "network",
      purpose: "Point every device at Pi-hole and ads disappear on phones, TVs and consoles without installing anything on them.",
      library: ["apps/pihole"],
      stacks: [
        {
          name: "dns",
          compose: "services:\n  pihole:\n    image: pihole/pihole:latest\n    environment:\n      FTLCONF_webserver_api_password: ${PIHOLE_PASSWORD}\n",
          files: [],
          env: [{ name: "PIHOLE_PASSWORD", label: "Dashboard password", default: "", help: "Leave empty to generate one.", secret: true, generate: true, path: false }],
          ports: [{ port: "53/udp", what: "DNS", local: false }],
        },
      ],
      firewall: { allow: ["53/udp"], limit: [], denyIncoming: false, enable: false },
      timers: [],
      manual: [],
    },
  ],
  runbooks: [
    {
      id: "disk-full",
      title: "Runbook: disk full or filling up",
      summary: "Find what grew, free space safely, and stop it happening again.",
      alertKinds: ["diskFull", "diskFilling"],
      tags: ["runbook", "starter", "disk"],
      library: ["linux/disk-full"],
      body: "A mount is almost full.\n\n- [ ] Find the biggest folders:\n\n```sh\nsudo du -xh --max-depth=2 / | sort -h | tail -20\n```\n\nMore in [Disk full](kb:linux/disk-full).",
    },
    {
      id: "high-memory",
      title: "Runbook: high memory use",
      summary: "Find what's using the memory.",
      alertKinds: [],
      tags: ["runbook", "starter", "memory"],
      library: ["linux/memory-limits"],
      body: "The host is short on memory.\n\n- [ ] Look at available memory:\n\n```sh\nfree -h\n```",
    },
  ],
};

export const mockStarters = {
  async list(): Promise<StarterCatalog> {
    return structuredClone(catalog);
  },
  async target(): Promise<StarterTarget> {
    return { parent: "/home/you/stacks", why: "a folder in the login user's home, created if needed" };
  },
  async open(id: string, _values: StarterValue[], parent: string): Promise<BlueprintSummary> {
    const s = catalog.blueprints.find((b) => b.id === id);
    if (!s) throw new Error("no such starter");
    if (!parent.startsWith("/")) throw new Error("use an absolute folder such as /opt/stacks");
    return {
      host: `starter: ${s.title}`,
      madeAt: Math.floor(Date.now() / 1000),
      os: "",
      stacks: s.stacks.map((st) => ({ name: st.name, dir: `${parent}/${st.name}`, files: [`${parent}/${st.name}/compose.yaml`], env: st.env.map((e) => e.name) })),
      crontabs: [],
      timers: s.timers.map((t) => t.name),
      firewall: s.firewall ? "any" : null,
      firewallRules: (s.firewall?.allow.length ?? 0) + (s.firewall?.limit.length ?? 0),
      proxies: [],
      notes: [],
      manual: s.manual.map((m) => m.title),
    };
  },
};
