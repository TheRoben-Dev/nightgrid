/**
 * Browser-only stand-in for the Docker commands: a fake engine per host with a
 * compose stack, ticking stats and logs. Used by `pnpm dev`, tests and
 * screenshots; never talks to a real daemon.
 */
import type {
  Action,
  ActionResult_Serialize as ActionResult,
  ContainerDetail_Serialize as ContainerDetail,
  ContainerStats,
  ContainerView_Serialize as ContainerView,
  EngineInfo,
  LogEvent_Serialize as LogEvent,
  LogLine_Serialize as LogLine,
  LogQuery_Deserialize as LogQuery,
  ComposeEvent_Serialize as ComposeEvent,
  ComposeOp,
  DeployInput,
  ImageUpdate_Serialize as ImageUpdate,
  RemoteImage_Serialize as RemoteImage,
  Snapshot_Serialize as Snapshot,
  Stack,
  StackDir,
} from "./bindings";

let snaps: Snapshot[] = [];
let nextSnap = 1;
const deployed: Stack[] = [];

const now = () => Math.floor(Date.now() / 1000);

interface Fake {
  view: ContainerView;
  env: string[];
}

const engines = new Map<number, Fake[]>();

function seed(): Fake[] {
  const c = (
    name: string,
    image: string,
    state: string,
    project: string | null,
    service: string | null,
    ports: [number, number | null][],
    health: string | null = null,
    env: string[] = ["TZ=Europe/Stockholm"],
  ): Fake => ({
    view: {
      id: Array.from(name).reduce((h, ch) => (h * 31 + ch.charCodeAt(0)) >>> 0, 7).toString(16).padStart(12, "0"),
      name,
      image,
      state,
      status: state === "running" ? `Up 3 hours${health ? ` (${health})` : ""}` : "Exited (0) 2 days ago",
      health,
      created: now() - 86400 * 4,
      ports: ports.map(([p, pub]) => ({ ip: pub ? "0.0.0.0" : null, private: p, public: pub, proto: "tcp" })),
      project,
      service,
      composeDir: project ? `/srv/${project}` : null,
      composeFiles: project ? [`/srv/${project}/compose.yaml`] : [],
      web: ports
        .filter(([, pub]) => pub !== null)
        .map(([p, pub]) => ({ port: pub as number, scheme: [443, 8443, 9443].includes(p) ? "https" : "http", path: "/", bind: null, labelled: false })),
    },
    env,
  });
  return [
    c("plex", "plexinc/pms-docker:latest", "running", "media", "plex", [[32400, 32400]], "healthy", [
      "TZ=Europe/Stockholm",
      "PLEX_CLAIM=claim-mock-not-a-real-token",
    ]),
    c("convertx", "ghcr.io/c4illin/convertx:latest", "running", "media", "convertx", [[3000, 3100]]),
    c("media-sonarr-1", "lscr.io/linuxserver/sonarr:latest", "exited", "media", "sonarr", [[8989, null]]),
    c("homepage", "ghcr.io/gethomepage/homepage:latest", "running", "dash", "homepage", [[3000, 3000]], "healthy"),
    c("amp", "mitchtalmadge/amp-dockerized:latest", "running", null, null, [[8080, 8080]], "starting", [
      "USERNAME=admin",
      "PASSWORD=mock-password",
    ]),
  ];
}

function list(hostId: number): Fake[] {
  let e = engines.get(hostId);
  if (!e) {
    e = seed();
    engines.set(hostId, e);
  }
  return e;
}

const find = (hostId: number, id: string) => list(hostId).find((c) => c.view.id === id || c.view.name === id);

const delay = <T>(v: T, ms = 120) => new Promise<T>((r) => setTimeout(() => r(v), ms));

const logStreams = new Map<number, ReturnType<typeof setInterval>>();
let nextStream = 1;

function fakeLine(name: string, i: number): LogLine {
  const err = i % 7 === 3;
  const msgs = [
    `GET /api/status 200 ${4 + (i % 9)}ms`,
    `worker ${i % 4}: job ${1000 + i} done`,
    `[${name}] heartbeat ok`,
    `WARN slow query took ${200 + i}ms`,
  ];
  return {
    stream: err ? "err" : "out",
    ts: new Date(Date.now() - 1000).toISOString().replace("Z", "000000Z"),
    text: err ? `error: upstream timeout (attempt ${i % 3})` : msgs[i % msgs.length],
  };
}

let stackDirs: StackDir[] = [];
let nextDir = 1;
const composeRuns = new Map<number, ReturnType<typeof setInterval>>();
let nextRun = 1;

function stacksOf(hostId: number): Stack[] {
  const by = new Map<string, Stack>();
  for (const { view: v } of list(hostId)) {
    if (!v.project) continue;
    const s = by.get(v.project) ?? {
      name: v.project,
      dir: v.composeDir ?? `/srv/${v.project}`,
      files: v.composeFiles,
      running: 0,
      total: 0,
      inFolder: false,
    };
    s.total++;
    if (v.state === "running") s.running++;
    by.set(v.project, s);
  }
  if (stackDirs.some((d) => d.hostId === hostId)) {
    by.set("vault", { name: "vault", dir: "/opt/stacks/vault", files: ["/opt/stacks/vault/compose.yaml"], running: 0, total: 0, inFolder: true });
    for (const s of by.values()) if (s.dir.startsWith("/srv/")) s.inFolder = true;
  }
  return [...by.values()].sort((a, b) => a.name.localeCompare(b.name));
}


export const mockDocker = {
  info: (hostId: number): Promise<EngineInfo> =>
    delay({ version: "27.5.1", apiVersion: "1.47", os: "linux/amd64", route: `socket /var/run/docker.sock over SSH (mock ${hostId})` }),

  containers: (hostId: number): Promise<ContainerView[]> => delay(list(hostId).map((c) => ({ ...c.view }))),

  stats: (hostId: number, ids: string[]): Promise<ContainerStats[]> =>
    delay(
      ids
        .map((id) => find(hostId, id))
        .filter((c): c is Fake => !!c && c.view.state === "running")
        .map((c, i) => ({
          id: c.view.id,
          cpuPermille: Math.round(Math.random() * 300 + i * 40),
          memUsed: (180 + i * 90 + Math.round(Math.random() * 20)) * 1024 * 1024,
          memLimit: 16 * 1024 * 1024 * 1024,
        })),
      300,
    ),

  action: (hostId: number, ids: string[], action: Action): Promise<ActionResult[]> => {
    const out: ActionResult[] = [];
    for (const id of ids) {
      const c = find(hostId, id);
      if (!c) {
        out.push({ id, error: `no such container: ${id}` });
        continue;
      }
      const v = c.view;
      switch (action.kind) {
        case "start":
        case "restart":
        case "unpause":
          v.state = "running";
          v.status = `Up 1 second${v.health ? " (health: starting)" : ""}`;
          if (v.health) v.health = "starting";
          break;
        case "stop":
        case "kill":
          v.state = "exited";
          v.status = `Exited (${action.kind === "kill" ? 137 : 0}) 1 second ago`;
          break;
        case "pause":
          v.state = "paused";
          v.status = "Up 3 hours (Paused)";
          break;
        case "remove":
          if (v.state === "running" && !action.force) {
            out.push({ id, error: `cannot remove running container ${v.name}: stop it first or force` });
            continue;
          }
          engines.set(
            hostId,
            list(hostId).filter((x) => x !== c),
          );
      }
      out.push({ id, error: null });
    }
    return delay(out, 400);
  },

  inspect: (hostId: number, id: string): Promise<ContainerDetail> => {
    const c = find(hostId, id);
    if (!c) return Promise.reject({ message: `no such container: ${id}`, connect: null, docker: { kind: "notFound", id } });
    const v = c.view;
    const env = c.env.map((e) => {
      const [key, ...rest] = e.split("=");
      return { key, value: rest.join("=") };
    });
    const labels = v.project
      ? [
          { key: "com.docker.compose.project", value: v.project },
          { key: "com.docker.compose.project.working_dir", value: v.composeDir ?? "" },
          { key: "com.docker.compose.service", value: v.service ?? "" },
        ]
      : [];
    const raw = JSON.stringify(
      {
        Id: v.id,
        Name: `/${v.name}`,
        State: { Status: v.state },
        Config: { Image: v.image, Env: env.map((e) => `${e.key}=••••••`), Labels: Object.fromEntries(labels.map((l) => [l.key, l.value])) },
      },
      null,
      2,
    );
    return delay({
      id: v.id,
      name: v.name,
      image: v.image,
      created: new Date((now() - 86400 * 4) * 1000).toISOString(),
      state: v.state,
      startedAt: v.state === "running" ? new Date((now() - 3 * 3600) * 1000).toISOString() : null,
      finishedAt: v.state === "exited" ? new Date((now() - 2 * 86400) * 1000).toISOString() : null,
      exitCode: v.state === "exited" ? 0 : null,
      health: v.health,
      restartPolicy: "unless-stopped",
      restartCount: 0,
      command: "/init",
      env,
      mounts: [
        { kind: "bind", source: `${v.composeDir ?? "/opt"}/config`, destination: "/config", rw: true, name: null },
        { kind: "volume", source: `/var/lib/docker/volumes/${v.name}_data/_data`, destination: "/data", rw: false, name: `${v.name}_data` },
      ],
      networks: [{ name: v.project ? `${v.project}_default` : "bridge", ip: "172.18.0.4", gateway: "172.18.0.1", aliases: [v.service ?? v.name] }],
      ports: v.ports,
      labels,
      raw,
    });
  },

  logsStart: (hostId: number, id: string, q: LogQuery, on: (e: LogEvent) => void): Promise<number> => {
    const c = find(hostId, id);
    const name = c?.view.name ?? id;
    const sid = nextStream++;
    const backlog = Math.min(q.tail ?? 200, 200);
    setTimeout(() => on({ kind: "lines", lines: Array.from({ length: backlog }, (_, i) => fakeLine(name, i)) }), 50);
    if (q.follow && c?.view.state === "running") {
      let i = backlog;
      logStreams.set(
        sid,
        setInterval(() => on({ kind: "lines", lines: [fakeLine(name, i++)] }), 700),
      );
    } else {
      setTimeout(() => on({ kind: "end", error: null }), 100);
    }
    return delay(sid, 30);
  },

  logsStop: (sid: number): Promise<null> => {
    clearInterval(logStreams.get(sid));
    logStreams.delete(sid);
    return delay(null, 0);
  },
  stacks: (hostId: number): Promise<Stack[]> => delay([...stacksOf(hostId), ...deployed.filter((d) => !stacksOf(hostId).some((s) => s.name === d.name))], 200),
  stackDirs: (hostId: number): Promise<StackDir[]> => delay(stackDirs.filter((d) => d.hostId === hostId)),
  stackDirAdd: (hostId: number, path: string): Promise<StackDir> => {
    const d = { id: nextDir++, hostId, path };
    stackDirs = [...stackDirs, d];
    return delay(d);
  },
  stackDirDelete: (id: number): Promise<null> => {
    stackDirs = stackDirs.filter((d) => d.id !== id);
    return delay(null);
  },
  composeRun: (hostId: number, stack: Stack, op: ComposeOp, on: (e: ComposeEvent) => void): Promise<number> => {
    const run = nextRun++;
    const svcs = list(hostId).filter((c) => c.view.project === stack.name);
    const script: string[] =
      op === "down"
        ? [...svcs.map((c) => ` Container ${c.view.name}  Stopping`), ...svcs.map((c) => ` Container ${c.view.name}  Removed`), ` Network ${stack.name}_default  Removed`]
        : op === "pull" || op === "update"
          ? [...svcs.map((c) => ` ${c.view.service} Pulling`), ...svcs.map((c) => ` ${c.view.service} Pulled`)]
          : svcs.map((c) => ` Container ${c.view.name}  Started`);
    if (op === "update") script.push("", "── pull done, recreating changed services (up -d) ──", ...svcs.map((c) => ` Container ${c.view.name}  Started`));
    let i = 0;
    composeRuns.set(
      run,
      setInterval(() => {
        if (i < script.length) {
          on({ kind: "lines", lines: [script[i++]] });
          return;
        }
        clearInterval(composeRuns.get(run));
        composeRuns.delete(run);
        for (const c of svcs) {
          if (op === "down") c.view.state = "exited";
          else if (op !== "pull") c.view.state = "running";
        }
        const changes = op === "update" && svcs[0] ? [{ service: svcs[0].view.service ?? "", image: svcs[0].view.image, before: "1a2b3c4d5e6f", after: "6f5e4d3c2b1a" }] : null;
        on({ kind: "done", code: 0, changes, error: null });
      }, 250),
    );
    return delay(run, 30);
  },
  snapshots: (hostId: number, project: string): Promise<Snapshot[]> =>
    delay(snaps.filter((s) => s.hostId === hostId && s.project === project).sort((a, b) => b.createdAt - a.createdAt || b.id - a.id)),
  snapshotTake: (hostId: number, stack: Stack, reason = "manual"): Promise<Snapshot> => {
    const svcs = list(hostId).filter((c) => c.view.project === stack.name);
    const s: Snapshot = {
      id: nextSnap++,
      hostId,
      project: stack.name,
      reason,
      services: svcs.map((c, i) => ({
        service: c.view.service ?? c.view.name,
        image: c.view.image,
        imageId: `sha256:${(nextSnap * 7919 + i).toString(16).padStart(12, "0")}${"0".repeat(52)}`,
        repoDigest: `${c.view.image.split(":")[0]}@sha256:${"ab".repeat(32)}`,
      })),
      files: [
        { path: `${stack.dir}/compose.yaml`, saved: `/home/admin/.local/state/nightgrid/snapshots/${stack.name}/${Date.now()}/1-compose.yaml` },
        { path: `${stack.dir}/.env`, saved: `/home/admin/.local/state/nightgrid/snapshots/${stack.name}/${Date.now()}/2-.env` },
      ],
      createdAt: Math.floor(Date.now() / 1000) - (reason === "seed" ? 86400 * 2 : 0),
    };
    snaps = [...snaps, s];
    return delay(s);
  },
  snapshotDelete: (id: number): Promise<null> => {
    snaps = snaps.filter((s) => s.id !== id);
    return delay(null);
  },
  rollback: (hostId: number, stack: Stack, snapshot: number, on: (e: ComposeEvent) => void): Promise<number> => {
    const snap = snaps.find((s) => s.id === snapshot);
    void mockDocker.snapshotTake(hostId, stack, "before rollback");
    const lines = [
      "Saved the current state as a snapshot first.",
      ...(snap?.files ?? []).map((f) => `restored ${f.path}`),
      ...(snap?.services ?? []).map((s) => `${s.service}: ${s.image} → ${s.imageId.slice(7, 19)}`),
      "",
      "── recreating changed services (up -d) ──",
      ...(snap?.services ?? []).map((s) => ` Container ${stack.name}-${s.service}-1  Started`),
    ];
    const run = nextRun++;
    let i = 0;
    composeRuns.set(
      run,
      setInterval(() => {
        if (i < lines.length) return on({ kind: "lines", lines: [lines[i++]] });
        clearInterval(composeRuns.get(run));
        composeRuns.delete(run);
        on({ kind: "done", code: 0, changes: null, error: null });
      }, 200),
    );
    return delay(run, 30);
  },
  checkUpdates: (hostId: number, stack: Stack): Promise<ImageUpdate[]> => {
    const images = [...new Map(list(hostId).filter((c) => c.view.project === stack.name).map((c) => [c.view.image, c.view.service ?? c.view.name])).entries()];
    return delay(
      images.map(([image, service], i): ImageUpdate => {
        const src = "https://github.com/example/" + image.split(/[/:]/).slice(-2, -1)[0];
        if (i % 3 === 0)
          return {
            image,
            services: [service],
            status: "available",
            error: null,
            current: { version: "1.4.2", revision: "a1b2c3d4", created: "2026-09-02T10:00:00Z" },
            latest: { version: "1.5.0", revision: "e5f6a7b8", created: "2026-11-10T08:30:00Z" },
            source: src,
            notes: `${src}/compare/a1b2c3d4...e5f6a7b8`,
          };
        if (i % 3 === 1)
          return { image, services: [service], status: "current", error: null, current: { version: "2.0.1", revision: null, created: "2026-10-20T00:00:00Z" }, latest: null, source: src, notes: `${src}/releases` };
        return { image, services: [service], status: "needsLogin", error: null, current: { version: null, revision: null, created: null }, latest: null, source: null, notes: null };
      }),
      700,
    );
  },
  imageLookup: (_hostId: number, image: string): Promise<RemoteImage> => {
    if (!image.trim() || /\s/.test(image)) return Promise.reject(new Error("not a valid image reference"));
    return delay(
      {
        image,
        version: { version: "3.2.0", revision: null, created: "2026-11-01T00:00:00Z" },
        ports: ["8080/tcp"],
        volumes: ["/config", "/data"],
        env: ["TZ", "PUID", "PGID", "ADMIN_TOKEN"],
        source: "https://github.com/example/app",
        description: "An example self-hosted web app.",
      },
      500,
    );
  },
  deploy: (_hostId: number, input: DeployInput): Promise<Stack> => {
    const dir = `${input.parent.replace(/\/+$/, "")}/${input.name}`;
    if (!/^[a-z0-9][a-z0-9_-]*$/.test(input.name)) return Promise.reject(new Error("name: lowercase letters, digits, - and _, starting with a letter or digit"));
    if (!input.overwrite && deployed.some((s) => s.dir === dir)) return Promise.reject(new Error(`already exists: ${dir}/compose.yaml`));
    const s: Stack = { name: input.name, dir, files: [], running: 0, total: 0, inFolder: true };
    if (!deployed.some((d) => d.dir === dir)) deployed.push(s);
    return delay(s, 600);
  },
  composeCancel: (run: number): Promise<null> => {
    clearInterval(composeRuns.get(run));
    composeRuns.delete(run);
    return delay(null, 0);
  },
};
