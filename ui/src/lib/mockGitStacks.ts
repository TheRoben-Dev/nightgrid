/**
 * Browser-only stand-in for stacks from git (M10w): one tracked repository
 * with two incoming commits, deployable and rollback-able in memory.
 */
import type {
  GitCommit,
  GitDeployed_Serialize as GitDeployed,
  GitStack_Serialize as GitStack,
  GitStackAdded_Serialize as GitStackAdded,
  GitStackInput_Deserialize as GitStackInput,
  GitStatus_Serialize as GitStatus,
  Stack,
} from "./bindings";
import { repoName, validBranch, validDir, validSubdir, validUrl } from "./gitstacks";

const sha = (c: string) => c.repeat(40).slice(0, 40);
const now = () => Math.floor(Date.now() / 1000);

/** Commits on the remote branch, oldest first. */
const history: GitCommit[] = [
  { id: sha("1a"), at: now() - 86400 * 9, author: "you", subject: "Initial media stack" },
  { id: sha("2b"), at: now() - 86400 * 2, author: "you", subject: "Add healthcheck to jellyfin" },
  { id: sha("3c"), at: now() - 3600 * 5, author: "you", subject: "Bump jellyfin to 10.10.3" },
];
const DIFF = `diff --git a/media/compose.yaml b/media/compose.yaml
index 4f2a1c0..9b7e3d2 100644
--- a/media/compose.yaml
+++ b/media/compose.yaml
@@ -1,8 +1,13 @@
 services:
   jellyfin:
-    image: jellyfin/jellyfin:10.9.11
+    image: jellyfin/jellyfin:10.10.3
     restart: unless-stopped
     ports:
       - "8096:8096"
+    healthcheck:
+      test: ["CMD", "curl", "-fsS", "http://localhost:8096/health"]
+      interval: 30s
+      retries: 3
     volumes:
       - ./config:/config
`;

let nextId = 2;
let list: GitStack[] = [
  {
    id: 1,
    hostId: 1,
    dir: "~/stacks/homelab",
    url: "git@git.example.com:me/homelab.git",
    branch: "main",
    subdir: "media",
    deployed: sha("1a"),
    previous: null,
    deployedAt: now() - 86400 * 9,
  },
];
/** Checked-out commit per stack. */
const heads = new Map<number, string>([[1, sha("1a")]]);

const stackOf = (g: GitStack): Stack => {
  const dir = (g.subdir ? `${g.dir}/${g.subdir}` : g.dir).replace(/^~/, "/home/admin");
  const name = (dir.split("/").pop() ?? "stack").toLowerCase();
  return { name, dir, files: [`${dir}/compose.yaml`], running: 0, total: 0, inFolder: true };
};

function find(id: number): GitStack {
  const g = list.find((x) => x.id === id);
  if (!g) throw new Error("git stack not found");
  return g;
}

export const mockGitStacks = {
  list: async (hostId: number): Promise<GitStack[]> => list.filter((g) => g.hostId === hostId),
  add: async (hostId: number, i: GitStackInput): Promise<GitStackAdded> => {
    const url = i.url.trim();
    const dir = i.dir.trim().replace(/\/+$/, "");
    const subdir = (i.subdir ?? "").trim().replace(/^\/+|\/+$/g, "");
    if (!validUrl(url)) throw new Error("invalid value for url: use an https://, ssh:// or user@host:path address");
    if (!validBranch(i.branch.trim())) throw new Error("invalid value for branch");
    if (!validDir(dir)) throw new Error("invalid value for dir: use a folder on the host: an absolute path or one starting with ~/");
    if (!validSubdir(subdir)) throw new Error("invalid value for subdir");
    if (list.some((g) => g.hostId === hostId && g.dir === dir && g.subdir === subdir)) throw new Error("invalid value for dir: that folder is already tracked");
    if (url.includes("private")) throw new Error("git@git.example.com: Permission denied (publickey).\nThe host has no SSH key this repository accepts. Add the host's public key as a deploy key.");
    const g: GitStack = { id: nextId++, hostId, dir, url, branch: i.branch.trim(), subdir, deployed: null, previous: null, deployedAt: null };
    list = [...list, g];
    heads.set(g.id, history[history.length - 1].id);
    return { git: g, adopted: false, stack: repoName(url) === "empty" ? null : stackOf(g) };
  },
  check: async (id: number): Promise<GitStatus> => {
    const g = find(id);
    const head = heads.get(id) ?? history[0].id;
    const at = history.findIndex((c) => c.id === head);
    const incoming = history.slice(at + 1).reverse();
    return {
      branch: g.branch,
      head,
      current: history[at] ?? null,
      remote: history[history.length - 1].id,
      behind: incoming.length,
      ahead: 0,
      incoming,
      files: incoming.length ? [{ path: g.subdir ? `${g.subdir}/compose.yaml` : "compose.yaml", added: 6, removed: 1 }] : [],
      dirty: [],
      stackDir: stackOf(g).dir,
      composeFile: stackOf(g).files[0],
      error: null,
    };
  },
  diff: async (id: number, target: string): Promise<string> => {
    find(id);
    const head = heads.get(id);
    const a = history.findIndex((c) => c.id === head);
    const b = history.findIndex((c) => c.id === target);
    if (b < 0) throw new Error("that commit isn't on the host; check for changes again");
    if (a === b) return "";
    // Going back shows the same change reversed.
    return b > a ? DIFF : DIFF.split("\n").map((l) => (l.startsWith("+") && !l.startsWith("+++") ? "-" + l.slice(1) : l.startsWith("-") && !l.startsWith("---") ? "+" + l.slice(1) : l)).join("\n");
  },
  deploy: async (id: number, target: string): Promise<GitDeployed> => {
    const g = find(id);
    if (!history.some((c) => c.id === target)) throw new Error("that commit isn't on the tracked branch");
    const from = heads.get(id) ?? null;
    heads.set(id, target);
    const prev = g.deployed && g.deployed !== target ? g.deployed : g.previous;
    const next: GitStack = { ...g, deployed: target, previous: prev, deployedAt: now() };
    list = list.map((x) => (x.id === id ? next : x));
    return { git: next, from, to: target, stack: stackOf(next), snapshot: true, snapshotError: null };
  },
  remove: async (id: number): Promise<null> => {
    list = list.filter((g) => g.id !== id);
    return null;
  },
};
