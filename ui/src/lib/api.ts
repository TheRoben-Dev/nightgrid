/**
 * Backend access. Inside Tauri this calls the generated, typed commands; in a
 * plain browser (`pnpm dev` without Tauri, tests, screenshots) it falls back
 * to an in-memory/localStorage mock so the UI can be developed standalone.
 */
import { Channel, isTauri } from "@tauri-apps/api/core";
import {
  commands,
  type AppInfo_Serialize as AppInfo,
  type AppSettings_Serialize as AppSettings,
  type AuthMethod,
  type ConnectError,
  type Group_Serialize as Group,
  type GroupInput_Serialize as GroupInput,
  type Host_Serialize as Host,
  type HostInput_Serialize as HostInput,
  type HostKeyInfo,
  type HostPing_Serialize as HostPing,
  type ImportResult,
  type PromptKind,
  type SshConfigCandidate_Serialize as SshConfigCandidate,
  type TermMessage_Serialize as TermMessage,
  type TermOpen,
  type TermState,
  type Bookmark_Serialize as Bookmark,
  type ConflictPolicy,
  type Entry_Serialize as Entry,
  type EntryKind,
  type FilesError_Serialize as FilesError,
  type FsError_Serialize as FsError,
  type JobState,
  type JobView_Serialize as JobView,
  type Listing_Serialize as Listing,
  type NewJob_Deserialize as NewJob,
  type SaveResult_Serialize as SaveResult,
  type Side,
  type Stamp_Deserialize as Stamp,
  type TextFile_Serialize as TextFile,
  type Action as DockerAction,
  type ActionResult_Serialize as ActionResult,
  type ContainerDetail_Serialize as ContainerDetail,
  type ContainerStats,
  type ContainerView_Serialize as ContainerView,
  type DockerError,
  type EngineInfo,
  type LogEvent_Serialize as LogEvent,
  type LogLine_Serialize as LogLine,
  type LogQuery_Deserialize as LogQuery,
  type ComposeEvent_Serialize as ComposeEvent,
  type ComposeOp,
  type ImageChange_Serialize as ImageChange,
  type Stack,
  type StackDir,
} from "./bindings";
import { mockBackend } from "./mock";
import { mockDocker } from "./mockDocker";
import { mockFiles } from "./mockFiles";

export type {
  AppInfo,
  AppSettings,
  AuthMethod,
  ConnectError,
  Group,
  GroupInput,
  Host,
  HostInput,
  HostKeyInfo,
  HostPing,
  ImportResult,
  PromptKind,
  SshConfigCandidate,
  TermMessage,
  TermOpen,
  TermState,
  Bookmark,
  ConflictPolicy,
  Entry,
  EntryKind,
  FilesError,
  FsError,
  JobState,
  JobView,
  Listing,
  NewJob,
  SaveResult,
  Side,
  Stamp,
  TextFile,
  DockerAction,
  ActionResult,
  ContainerDetail,
  ContainerStats,
  ContainerView,
  DockerError,
  EngineInfo,
  LogEvent,
  LogLine,
  LogQuery,
  ComposeEvent,
  ComposeOp,
  ImageChange,
  Stack,
  StackDir,
};

export const DEFAULT_SETTINGS: AppSettings = {
  theme: "nightgrid",
  reduceEffects: false,
  scanlines: true,
  uiScale: 100,
  railCollapsed: false,
  termFontSize: 13,
  termScrollback: 10000,
  termCopyOnSelect: true,
  termRightClickPaste: true,
  termCursorBlink: true,
  termEditInApp: false,
};

const MOCK_KEY = "nightgrid.mock.settings";

const mock = {
  async appInfo(): Promise<AppInfo> {
    return {
      version: "0.3.0-web",
      os: "browser",
      arch: "n/a",
      dataDir: "(browser)",
      logDir: "(browser)",
      schemaVersion: 1,
      keychain: { available: false, detail: "running outside Tauri" },
      dmabufDisabled: false,
      agentHint: null,
    };
  },
  async settingsGet(): Promise<AppSettings> {
    try {
      const raw = localStorage.getItem(MOCK_KEY);
      return raw ? { ...DEFAULT_SETTINGS, ...JSON.parse(raw) } : { ...DEFAULT_SETTINGS };
    } catch {
      return { ...DEFAULT_SETTINGS };
    }
  },
  async settingsSet(s: AppSettings): Promise<AppSettings> {
    try {
      localStorage.setItem(MOCK_KEY, JSON.stringify(s));
    } catch {
      /* storage unavailable: keep in memory only */
    }
    return s;
  },
};

export const inTauri = (): boolean => {
  try {
    return isTauri();
  } catch {
    return false;
  }
};

/** Receives terminal events. Returns a function that detaches it. */
export type TermListener = (m: TermMessage) => void;

function channel(listener: TermListener) {
  const ch = new Channel<TermMessage>();
  ch.onmessage = listener;
  // The generated signature names the deserialize-side type; they are identical.
  return ch as unknown as Parameters<typeof commands.termOpenLocal>[2];
}

const T = inTauri;

function jobsChannel(listener: (jobs: JobView[]) => void) {
  const ch = new Channel<JobView[]>();
  ch.onmessage = listener;
  return ch as unknown as Parameters<typeof commands.transfersSubscribe>[0];
}


export const api = {
  appInfo: (): Promise<AppInfo> => (T() ? commands.appInfo() : mock.appInfo()),
  settingsGet: (): Promise<AppSettings> => (T() ? commands.settingsGet() : mock.settingsGet()),
  settingsSet: (s: AppSettings): Promise<AppSettings> => (T() ? commands.settingsSet(s) : mock.settingsSet(s)),

  hostsList: (): Promise<Host[]> => (T() ? commands.hostsList() : mockBackend.hostsList()),
  hostCreate: (i: HostInput): Promise<Host> => (T() ? commands.hostCreate(i) : mockBackend.hostCreate(i)),
  hostUpdate: (id: number, i: HostInput): Promise<Host> =>
    T() ? commands.hostUpdate(id, i) : mockBackend.hostUpdate(id, i),
  hostDelete: (id: number): Promise<null> => (T() ? commands.hostDelete(id) : mockBackend.hostDelete(id)),
  hostForgetSecrets: (id: number): Promise<null> => (T() ? commands.hostForgetSecrets(id) : Promise.resolve(null)),
  groupsList: (): Promise<Group[]> => (T() ? commands.groupsList() : mockBackend.groupsList()),
  groupCreate: (i: GroupInput): Promise<Group> => (T() ? commands.groupCreate(i) : mockBackend.groupCreate(i)),
  groupDelete: (id: number): Promise<null> => (T() ? commands.groupDelete(id) : mockBackend.groupDelete(id)),
  sshConfigScan: (): Promise<SshConfigCandidate[]> => (T() ? commands.sshConfigScan() : mockBackend.sshConfigScan()),
  sshConfigImport: (aliases: string[]): Promise<ImportResult> =>
    T() ? commands.sshConfigImport(aliases) : mockBackend.sshConfigImport(aliases),
  knownHostsImportSystem: (): Promise<number> => (T() ? commands.knownHostsImportSystem() : Promise.resolve(0)),
  hostsPing: (ids: number[]): Promise<HostPing[]> => (T() ? commands.hostsPing(ids) : mockBackend.hostsPing(ids)),

  termOpenLocal: (cols: number, rows: number, on: TermListener): Promise<number> =>
    T() ? commands.termOpenLocal(cols, rows, channel(on)) : mockBackend.termOpenLocal(on),
  termOpenSsh: (hostId: number, cols: number, rows: number, editToken: string | null, on: TermListener): Promise<TermOpen> =>
    T() ? commands.termOpenSsh(hostId, cols, rows, editToken, channel(on)) : mockBackend.termOpenSsh(hostId, on),
  termWrite: (id: number, data: string): Promise<null> =>
    T() ? commands.termWrite(id, data) : mockBackend.termWrite(id, data),
  termResize: (id: number, cols: number, rows: number): Promise<null> =>
    T() ? commands.termResize(id, cols, rows) : Promise.resolve(null),
  termReconnect: (id: number): Promise<null> => (T() ? commands.termReconnect(id) : Promise.resolve(null)),
  termClose: (id: number): Promise<null> => (T() ? commands.termClose(id) : mockBackend.termClose(id)),
  sshTrustHostKey: (host: string, port: number, openssh: string, replace: boolean): Promise<null> =>
    T() ? commands.sshTrustHostKey(host, port, openssh, replace) : mockBackend.trust(host),
  sshProvideSecret: (target: string, kind: PromptKind, secret: string, remember: boolean): Promise<null> =>
    T() ? commands.sshProvideSecret(target, kind, secret, remember) : mockBackend.provideSecret(target),

  fsHome: (side: Side): Promise<string> => (T() ? commands.fsHome(side) : mockFiles.home(side)),
  fsList: (side: Side, path: string): Promise<Listing> => (T() ? commands.fsList(side, path) : mockFiles.list(side, path)),
  fsMkdir: (side: Side, dir: string, name: string): Promise<string> =>
    T() ? commands.fsMkdir(side, dir, name) : mockFiles.mkdir(side, dir, name),
  fsRename: (side: Side, path: string, newName: string): Promise<string> =>
    T() ? commands.fsRename(side, path, newName) : mockFiles.rename(side, path, newName),
  fsDelete: (side: Side, paths: string[]): Promise<null> =>
    T() ? commands.fsDelete(side, paths) : mockFiles.remove(side, paths),
  fsReadText: (side: Side, path: string): Promise<TextFile> =>
    T() ? commands.fsReadText(side, path) : mockFiles.readText(side, path),
  fsSaveText: (side: Side, path: string, content: string, expect: Stamp | null): Promise<SaveResult> =>
    T() ? commands.fsSaveText(side, path, content, expect) : mockFiles.saveText(side, path, content, expect),
  bookmarksList: (hostId: number | null): Promise<Bookmark[]> =>
    T() ? commands.bookmarksList(hostId) : mockFiles.bookmarks(hostId),
  bookmarkAdd: (hostId: number | null, path: string, label: string | null): Promise<Bookmark> =>
    T() ? commands.bookmarkAdd(hostId, path, label) : mockFiles.bookmarkAdd(hostId, path, label),
  bookmarkDelete: (id: number): Promise<null> => (T() ? commands.bookmarkDelete(id) : mockFiles.bookmarkDelete(id)),
  transfersSubscribe: (on: (jobs: JobView[]) => void): Promise<null> =>
    T() ? commands.transfersSubscribe(jobsChannel(on)) : mockFiles.subscribe(on),
  transfersAdd: (jobs: NewJob[]): Promise<number[]> => (T() ? commands.transfersAdd(jobs) : mockFiles.add(jobs)),
  transferPause: (id: number): Promise<null> => (T() ? commands.transferPause(id) : mockFiles.control(id, "pause")),
  transferResume: (id: number): Promise<null> => (T() ? commands.transferResume(id) : mockFiles.control(id, "resume")),
  transferCancel: (id: number): Promise<null> => (T() ? commands.transferCancel(id) : mockFiles.control(id, "cancel")),
  transferResolve: (id: number, policy: ConflictPolicy, all: boolean): Promise<null> =>
    T() ? commands.transferResolve(id, policy, all) : mockFiles.resolve(id, policy, all),
  transfersClear: (): Promise<null> => (T() ? commands.transfersClear() : mockFiles.clear()),

  dockerInfo: (hostId: number): Promise<EngineInfo> => (T() ? commands.dockerInfo(hostId) : mockDocker.info(hostId)),
  dockerContainers: (hostId: number): Promise<ContainerView[]> =>
    T() ? commands.dockerContainers(hostId) : mockDocker.containers(hostId),
  dockerStats: (hostId: number, ids: string[]): Promise<ContainerStats[]> =>
    T() ? commands.dockerStats(hostId, ids) : mockDocker.stats(hostId, ids),
  dockerAction: (hostId: number, ids: string[], action: DockerAction): Promise<ActionResult[]> =>
    T() ? commands.dockerAction(hostId, ids, action) : mockDocker.action(hostId, ids, action),
  dockerInspect: (hostId: number, id: string): Promise<ContainerDetail> =>
    T() ? commands.dockerInspect(hostId, id) : mockDocker.inspect(hostId, id),
  dockerLogsStart: (hostId: number, id: string, query: LogQuery, on: (e: LogEvent) => void): Promise<number> => {
    if (!T()) return mockDocker.logsStart(hostId, id, query, on);
    const ch = new Channel<LogEvent>();
    ch.onmessage = on;
    return commands.dockerLogsStart(hostId, id, query, ch as unknown as Parameters<typeof commands.dockerLogsStart>[3]);
  },
  dockerLogsStop: (stream: number): Promise<null> => (T() ? commands.dockerLogsStop(stream) : mockDocker.logsStop(stream)),
  dockerForget: (hostId: number): Promise<null> => (T() ? commands.dockerForget(hostId) : Promise.resolve(null)),
  dockerStacks: (hostId: number): Promise<Stack[]> => (T() ? commands.dockerStacks(hostId) : mockDocker.stacks(hostId)),
  dockerComposeRun: (hostId: number, stack: Stack, op: ComposeOp, on: (e: ComposeEvent) => void): Promise<number> => {
    if (!T()) return mockDocker.composeRun(hostId, stack, op, on);
    const ch = new Channel<ComposeEvent>();
    ch.onmessage = on;
    return commands.dockerComposeRun(hostId, stack, op, ch as unknown as Parameters<typeof commands.dockerComposeRun>[3]);
  },
  dockerComposeCancel: (run: number): Promise<null> =>
    T() ? commands.dockerComposeCancel(run) : mockDocker.composeCancel(run),
  stackDirsList: (hostId: number): Promise<StackDir[]> =>
    T() ? commands.stackDirsList(hostId) : mockDocker.stackDirs(hostId),
  stackDirsAdd: (hostId: number, path: string): Promise<StackDir> =>
    T() ? commands.stackDirsAdd(hostId, path) : mockDocker.stackDirAdd(hostId, path),
  stackDirsDelete: (id: number): Promise<null> => (T() ? commands.stackDirsDelete(id) : mockDocker.stackDirDelete(id)),
  termOpenDockerExec: (hostId: number, container: string, cols: number, rows: number, on: TermListener): Promise<TermOpen> =>
    T()
      ? commands.termOpenDockerExec(hostId, container, cols, rows, channel(on))
      : mockBackend.termOpenSsh(hostId, on),
};

/** The structured parts of a rejected Docker command, when present. */
export function dockerError(e: unknown): { connect: ConnectError | null; docker: DockerError | null } {
  if (e && typeof e === "object") {
    const o = e as { connect?: ConnectError | null; docker?: DockerError | null };
    return { connect: o.connect ?? null, docker: o.docker ?? null };
  }
  return { connect: null, docker: null };
}

/** The structured parts of a rejected file command, when present. */
export function filesError(e: unknown): { connect: ConnectError | null; fs: FsError | null } {
  if (e && typeof e === "object") {
    const o = e as { connect?: ConnectError | null; fs?: FsError | null };
    return { connect: o.connect ?? null, fs: o.fs ?? null };
  }
  return { connect: null, fs: null };
}

/** Error text from a rejected command (`CommandError` or anything else). */
export function errorText(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}
