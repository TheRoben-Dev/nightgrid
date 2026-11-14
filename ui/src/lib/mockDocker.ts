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
} from "./bindings";

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
};
