/**
 * Browser-only stand-in for the file commands: an in-memory tree per side
 * (local, and one per host) with fake transfers. Used by `pnpm dev`, tests
 * and screenshots; never touches a real filesystem.
 */
import type {
  Bookmark_Serialize as Bookmark,
  ConflictPolicy,
  Entry_Serialize as Entry,
  FilesError_Serialize as FilesError,
  JobView_Serialize as JobView,
  Listing_Serialize as Listing,
  NewJob_Deserialize as NewJob,
  SaveResult_Serialize as SaveResult,
  Side,
  Stamp_Deserialize as Stamp,
  TextFile_Serialize as TextFile,
} from "./bindings";

interface Node {
  dir: boolean;
  content: string;
  size: number;
  mtime: number;
  mode: number;
}

const now = () => Math.floor(Date.now() / 1000);
const trees = new Map<string, Map<string, Node>>();
const sideKey = (s: Side) => (s.kind === "local" ? "local" : `host:${s.id}`);

function seed(local: boolean): Map<string, Node> {
  const t = new Map<string, Node>();
  const d = (p: string) => t.set(p, { dir: true, content: "", size: 4096, mtime: now() - 86400, mode: 0o755 });
  const f = (p: string, content: string, mode = 0o644, size?: number) =>
    t.set(p, { dir: false, content, size: size ?? content.length, mtime: now() - 3600, mode });
  if (local) {
    d("/Users");
    d("/Users/me");
    d("/Users/me/Downloads");
    d("/Users/me/Projects");
    f("/Users/me/Downloads/ubuntu-24.04-live-server-amd64.iso", "", 0o644, 2_754_981_888);
    f("/Users/me/Downloads/backup-2026-11-01.tar.zst", "", 0o644, 412_331_002);
    f("/Users/me/notes.md", "# Homelab notes\n\n- rotate keys\n- update stacks\n");
    f("/Users/me/.zshrc", "export EDITOR=nvim\n");
  } else {
    d("/home");
    d("/home/admin");
    d("/opt");
    d("/opt/stacks");
    d("/opt/stacks/media");
    d("/opt/stacks/homepage");
    d("/srv");
    d("/srv/media");
    d("/etc");
    f(
      "/opt/stacks/media/compose.yaml",
      "services:\n  jellyfin:\n    image: jellyfin/jellyfin:latest\n    restart: unless-stopped\n    ports:\n      - \"8096:8096\"\n    volumes:\n      - ./config:/config\n      - /srv/media:/media:ro\n",
    );
    f("/opt/stacks/media/.env", "TZ=Europe/Stockholm\nPUID=1000\nPGID=1000\n", 0o600);
    f("/opt/stacks/homepage/services.yaml", "- Media:\n    - Jellyfin:\n        href: http://lumen:8096\n");
    f("/home/admin/.bashrc", "alias ll='ls -la'\n");
    f("/home/admin/backup.sh", "#!/bin/sh\nset -eu\nrestic backup /opt/stacks\n", 0o755);
    f("/etc/hostname", "lumen\n");
    f("/srv/media/movie.mkv", "", 0o644, 8_412_000_000);
  }
  return t;
}

function tree(side: Side): Map<string, Node> {
  const k = sideKey(side);
  let t = trees.get(k);
  if (!t) {
    t = seed(side.kind === "local");
    trees.set(k, t);
  }
  return t;
}

const parentOf = (p: string): string | null => {
  const t = p.replace(/\/+$/, "");
  if (!t) return null;
  const i = t.lastIndexOf("/");
  return i <= 0 ? "/" : t.slice(0, i);
};
const join = (d: string, n: string) => (d.endsWith("/") ? d + n : `${d}/${n}`);
const nameOf = (p: string) => p.replace(/\/+$/, "").split("/").pop() || "/";
const home = (side: Side) => (side.kind === "local" ? "/Users/me" : "/home/admin");

function err(kind: string, path: string, message: string): FilesError {
  return { message, connect: null, fs: { kind, path } as FilesError["fs"] };
}

function entry(path: string, n: Node): Entry {
  return {
    name: nameOf(path),
    path,
    kind: n.dir ? "dir" : "file",
    targetKind: null,
    size: n.size,
    mtime: n.mtime,
    mode: n.mode,
    uid: 1000,
    gid: 1000,
  };
}

function resolve(side: Side, path: string): string {
  if (path === "~" || path === "" || path === ".") return home(side);
  if (path.startsWith("~/")) return join(home(side), path.slice(2));
  return path.length > 1 ? path.replace(/\/+$/, "") : path;
}

function children(t: Map<string, Node>, dir: string): string[] {
  return [...t.keys()].filter((p) => p !== dir && parentOf(p) === dir);
}

const delay = <T>(v: T, ms = 60) => new Promise<T>((r) => setTimeout(() => r(v), ms));

// --- transfers ---------------------------------------------------------------

let jobs: JobView[] = [];
let nextJob = 1;
let listener: ((j: JobView[]) => void) | null = null;
const emit = () => listener?.(jobs.map((j) => ({ ...j })));
let ticking = false;

function tick() {
  if (ticking) return;
  ticking = true;
  const timer = setInterval(() => {
    const running = jobs.filter((j) => j.state === "running");
    const queued = jobs.filter((j) => j.state === "queued");
    for (const j of queued.slice(0, Math.max(0, 3 - running.length))) startJob(j);
    for (const j of jobs.filter((x) => x.state === "running")) {
      const step = Math.max(1, Math.round(j.bytesTotal / 12));
      j.bytesDone = Math.min(j.bytesTotal, j.bytesDone + step);
      j.speed = step * 4;
      if (j.bytesDone >= j.bytesTotal) finishJob(j);
    }
    emit();
    if (!jobs.some((j) => j.state === "running" || j.state === "queued")) {
      clearInterval(timer);
      ticking = false;
    }
  }, 250);
}

function startJob(j: JobView) {
  const src = tree(j.src);
  const dst = tree(j.dst);
  const n = src.get(j.srcPath);
  if (!n) {
    j.state = "failed";
    j.error = `${j.srcPath}: no such file or directory`;
    return;
  }
  let target = join(j.dstDir, j.name);
  const pol = (j as JobView & { policy?: ConflictPolicy }).policy ?? "ask";
  if (dst.has(target) && j.bytesDone === 0) {
    if (pol === "ask") {
      j.state = "conflict";
      j.conflict = { src: entry(j.srcPath, n), dst: entry(target, dst.get(target)!) };
      return;
    }
    if (pol === "skip") {
      j.state = "skipped";
      return;
    }
    if (pol === "rename") {
      let i = 1;
      const dot = j.name.lastIndexOf(".");
      const mk = (k: number) => (dot > 0 ? `${j.name.slice(0, dot)} (${k})${j.name.slice(dot)}` : `${j.name} (${k})`);
      while (dst.has(join(j.dstDir, mk(i)))) i++;
      j.name = mk(i);
      target = join(j.dstDir, j.name);
    }
  }
  const files = n.dir ? [...src.keys()].filter((p) => p.startsWith(j.srcPath + "/")) : [j.srcPath];
  j.isDir = n.dir;
  j.filesTotal = files.filter((p) => !src.get(p)!.dir).length;
  j.bytesTotal = files.reduce((a, p) => a + (src.get(p)!.dir ? 0 : src.get(p)!.size), 0) || 1;
  j.state = "running";
  j.current = target;
}

function finishJob(j: JobView) {
  const src = tree(j.src);
  const dst = tree(j.dst);
  const target = join(j.dstDir, j.name);
  for (const [p, n] of [...src.entries()]) {
    if (p === j.srcPath || p.startsWith(j.srcPath + "/")) dst.set(target + p.slice(j.srcPath.length), { ...n, mtime: now() });
  }
  j.state = "done";
  j.filesDone = j.filesTotal;
  j.speed = 0;
  j.current = null;
}

// --- bookmarks ---------------------------------------------------------------

let bookmarks: Bookmark[] = [
  { id: 1, hostId: null, path: "/Users/me/Downloads", label: "Downloads" },
];
let nextBm = 2;

export const mockFiles = {
  async home(side: Side): Promise<string> {
    return delay(home(side));
  },
  async list(side: Side, path: string): Promise<Listing> {
    const t = tree(side);
    const p = resolve(side, path);
    const n = t.get(p);
    if (p !== "/" && !n) throw err("notFound", p, `${p}: no such file or directory`);
    if (n && !n.dir) throw err("other", p, `${p}: not a directory`);
    if (side.kind === "host" && p === "/etc/ssl/private") throw err("permissionDenied", p, `${p}: permission denied`);
    const entries = children(t, p)
      .map((c) => entry(c, t.get(c)!))
      .sort((a, b) => Number(b.kind === "dir") - Number(a.kind === "dir") || a.name.localeCompare(b.name));
    return delay({ path: p, parent: parentOf(p), entries });
  },
  async mkdir(side: Side, dir: string, name: string): Promise<string> {
    const t = tree(side);
    const p = join(dir, name);
    if (t.has(p)) throw err("exists", p, `${p}: already exists`);
    t.set(p, { dir: true, content: "", size: 4096, mtime: now(), mode: 0o755 });
    return delay(p);
  },
  async rename(side: Side, path: string, newName: string): Promise<string> {
    const t = tree(side);
    const to = join(parentOf(path) ?? "/", newName);
    if (t.has(to)) throw err("exists", to, `${to}: already exists`);
    for (const [p, n] of [...t.entries()]) {
      if (p === path || p.startsWith(path + "/")) {
        t.delete(p);
        t.set(to + p.slice(path.length), n);
      }
    }
    return delay(to);
  },
  async remove(side: Side, paths: string[]): Promise<null> {
    const t = tree(side);
    for (const path of paths) for (const p of [...t.keys()]) if (p === path || p.startsWith(path + "/")) t.delete(p);
    return delay(null);
  },
  async readText(side: Side, path: string): Promise<TextFile> {
    const n = tree(side).get(path);
    if (!n) throw err("notFound", path, `${path}: no such file or directory`);
    if (n.size > 5 * 1024 * 1024)
      throw { message: `${path} is too large for the editor`, connect: null, fs: { kind: "tooLarge", path, size: n.size, limit: 5242880 } };
    return delay({ path, content: n.content, stat: entry(path, n) });
  },
  async saveText(side: Side, path: string, content: string, expect: Stamp | null): Promise<SaveResult> {
    const t = tree(side);
    const n = t.get(path);
    if (n && expect && (n.size !== expect.size || n.mtime !== expect.mtime))
      throw { message: `${path} changed since it was opened`, connect: null, fs: { kind: "changed", path, current: entry(path, n) } };
    const next: Node = { dir: false, content, size: content.length, mtime: now() + 1, mode: n?.mode ?? 0o644 };
    t.set(path, next);
    return delay({ stat: entry(path, next), atomic: true });
  },
  bookmarks(hostId: number | null): Promise<Bookmark[]> {
    return delay(bookmarks.filter((b) => b.hostId === hostId));
  },
  bookmarkAdd(hostId: number | null, path: string, label: string | null): Promise<Bookmark> {
    const b = { id: nextBm++, hostId, path, label: label || nameOf(path) };
    bookmarks = [...bookmarks.filter((x) => !(x.hostId === hostId && x.path === path)), b];
    return delay(b);
  },
  bookmarkDelete(id: number): Promise<null> {
    bookmarks = bookmarks.filter((b) => b.id !== id);
    return delay(null);
  },
  subscribe(l: (j: JobView[]) => void): Promise<null> {
    listener = l;
    emit();
    return delay(null);
  },
  add(newJobs: NewJob[]): Promise<number[]> {
    const ids: number[] = [];
    for (const n of newJobs) {
      const id = nextJob++;
      ids.push(id);
      jobs.push({
        id,
        src: n.src,
        srcPath: n.srcPath,
        dst: n.dst,
        dstDir: n.dstDir,
        name: nameOf(n.srcPath),
        isDir: false,
        state: "queued",
        bytesDone: 0,
        bytesTotal: 0,
        filesDone: 0,
        filesTotal: 0,
        speed: 0,
        error: null,
        conflict: null,
        current: null,
        policy: n.policy ?? "ask",
      } as JobView);
    }
    emit();
    tick();
    return delay(ids);
  },
  control(id: number, op: "pause" | "resume" | "cancel"): Promise<null> {
    const j = jobs.find((x) => x.id === id);
    if (j) {
      if (op === "pause" && (j.state === "running" || j.state === "queued")) j.state = "paused";
      if (op === "resume" && (j.state === "paused" || j.state === "failed")) j.state = "queued";
      if (op === "cancel" && !["done", "skipped", "cancelled"].includes(j.state)) j.state = "cancelled";
      emit();
      tick();
    }
    return delay(null);
  },
  resolve(id: number, policy: ConflictPolicy, all: boolean): Promise<null> {
    for (const j of jobs) {
      if (j.id === id || (all && j.state === "conflict")) {
        (j as JobView & { policy?: ConflictPolicy }).policy = policy;
        if (j.state === "conflict") {
          j.state = "queued";
          j.conflict = null;
        }
      }
    }
    emit();
    tick();
    return delay(null);
  },
  clear(): Promise<null> {
    jobs = jobs.filter((j) => !["done", "skipped", "cancelled"].includes(j.state));
    emit();
    return delay(null);
  },
};
