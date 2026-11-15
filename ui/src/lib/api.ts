/**
 * Backend access. Inside Tauri this calls the generated, typed commands; in a
 * plain browser (`pnpm dev` without Tauri, tests, screenshots) it falls back
 * to an in-memory/localStorage mock so the UI can be developed standalone.
 */
import { Channel, isTauri } from "@tauri-apps/api/core";
import {
  type ChatTurn,
  type DownloadProgress,
  type HelperEvent,
  type HelperReply,
  type HelperStatus,
  commands,
  type BackupFolder_Serialize as BackupFolder,
  type BackupImported,
  type BackupPreview,
  type BackupSummary,
  type BackupWritten,
  type ImportMode,
  type Alert_Serialize as Alert,
  type AlertKind,
  type AlertSettings_Serialize as AlertSettings,
  type AlertsSnapshot_Serialize as AlertsSnapshot,
  type Severity,
  type AmpAction,
  type AutomationEvent_Serialize as AutomationEvent,
  type DockerAction as AutomationDockerAction,
  type HostNeeds,
  type OutLine,
  type OutStream,
  type Playbook_Serialize as Playbook,
  type PlaybookInput_Serialize as PlaybookInput,
  type RunInspect_Serialize as RunInspect,
  type RunSpec,
  type Snippet,
  type SnippetInput_Serialize as SnippetInput,
  type SnippetScope,
  type Step_Serialize as Step,
  type Variable_Serialize as Variable,
  type AmpInstance_Serialize as AmpInstance,
  type Integration_Serialize as Integration,
  type IntegrationInput_Serialize as IntegrationInput,
  type IntegrationKind,
  type IntegrationStatus_Serialize as IntegrationStatus,
  type IntegrationView_Serialize as IntegrationView,
  type KumaMonitor_Serialize as KumaMonitor,
  type PlexStream_Serialize as PlexStream,
  type TestResult_Serialize as AlertTestResult,
  type WebhookInfo_Serialize as WebhookInfo,
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
  type ImageUpdate_Serialize as ImageUpdate,
  type ImageStatus,
  type VersionInfo_Serialize as VersionInfo,
  type RemoteImage_Serialize as RemoteImage,
  type DeployInput,
  type Snapshot_Serialize as Snapshot,
  type SnapshotService,
  type SnapshotFile,
  type Stack,
  type StackDir,
  type MetricsEvent_Serialize as MetricsEvent,
  type MountUsage,
  type Point_Serialize as MetricPoint,
  type PollState,
  type Rate,
  type Sample_Serialize as Sample,
  type Temp,
  type UpdateInfo,
  type UpdateProgress_Serialize as UpdateProgress,
  type UpdateStatus_Serialize as UpdateStatus,
  type Service_Serialize as Service,
  type ServiceInput_Deserialize as ServiceInput,
  type ServiceStatus_Serialize as ServiceStatus,
  type ServiceEvent_Serialize as ServiceEvent,
  type CheckPoint_Serialize as CheckPoint,
  type CheckResult_Serialize as CheckResult,
  type TlsInfo_Serialize as TlsInfo,
  type HomepageCandidate,
  type HomepageEntry_Serialize as HomepageEntry,
  type HomepagePreview_Serialize as HomepagePreview,
  type ImportSummary,
  type Tunnel,
  type TunnelInput_Deserialize as TunnelInput,
  type TunnelView,
  type TunnelEvent,
  type TunnelState,
  type TunnelKind,
  type StatsView as TunnelTraffic,
  type DisksReport_Serialize as DisksReport,
  type Disk_Serialize as Disk,
  type Health,
  type JournalEntry_Serialize as JournalEntry,
  type JournalEvent_Serialize as JournalEvent,
  type JournalQuery_Deserialize as JournalQuery,
  type Manager as PackageManager,
  type PackageUpdate_Serialize as PackageUpdate,
  type RunEvent_Serialize as RunEvent,
  type SmartAttr_Serialize as SmartAttr,
  type SudoInput,
  type SudoState,
  type SystemError,
  type SystemInfo,
  type Unit_Serialize as Unit,
  type Schedules_Serialize as Schedules,
  type HistoryEntry_Serialize as HistoryEntry,
  type PowerAction,
  type WakeSent_Serialize as WakeSent,
  type HistoryResult_Serialize as HistoryResult,
  type Note_Serialize as Note,
  type NoteInput_Serialize as NoteInput,
  type BackupJob,
  type DeviceLabel,
  type TailnetStatus_Serialize as TailnetStatus,
  type TailnetPeer_Serialize as TailnetPeer,
  type BackupJobInput,
  type BackupStatus_Serialize as BackupStatus,
  type BackupSuggestion,
  type BackupTool,
  type BackupSource,
  type BackupResult,
  type ProxyReport_Serialize as ProxyReport,
  type Vm_Serialize as Vm,
  type VmKind,
  type VmAction,
  type VmReport_Serialize as VmReport,
  type Discovery_Serialize as Discovery,
  type Device_Serialize as Device,
  type Exposure_Serialize as Exposure,
  type ExposureKind,
  type Iface,
  type AddrKind,
  type Seen,
  type StatusPageInfo_Serialize as StatusPageInfo,
  type StatusPageSettings_Serialize as StatusPageSettings,
  type ProxyRoute_Serialize as ProxyRoute,
  type ProxyFound,
  type ProxyKind,
  type CertInfo_Serialize as CertInfo,
  type SecurityReport_Serialize as SecurityReport,
  type Check_Serialize as SecurityCheck,
  type CheckStatus,
  type Listener_Serialize as Listener,
  type Scope as ListenScope,
  type Timer_Serialize as Timer,
  type CronJob_Serialize as CronJob,
  type Periodic,
  type Gpu_Serialize as Gpu,
  type UnitAction,
  type UpdatesReport_Serialize as UpdatesReport,
  type UpgradeOp,
} from "./bindings";
import { mockAlerts } from "./mockAlerts";
import { mockAutomation } from "./mockAutomation";
import { mockNotes } from "./mockNotes";
import { mockOps } from "./mockOps";
import { mockTailnet } from "./mockTailnet";
import { mockHelper } from "./mockHelper";
import { mockBackup } from "./mockBackup";
import { mockIntegrations } from "./mockIntegrations";
import { mockBackend } from "./mock";
import { mockDocker } from "./mockDocker";
import { mockFiles } from "./mockFiles";
import { mockMetrics } from "./mockMetrics";
import { mockServices } from "./mockServices";
import { mockTunnels } from "./mockTunnels";
import { mockSystem } from "./mockSystem";

export type {
  ChatTurn,
  DownloadProgress,
  HelperEvent,
  HelperReply,
  HelperStatus,
  BackupFolder,
  BackupImported,
  BackupPreview,
  BackupSummary,
  BackupWritten,
  ImportMode,
  AutomationEvent,
  AutomationDockerAction,
  HostNeeds,
  OutLine,
  OutStream,
  Playbook,
  PlaybookInput,
  RunInspect,
  RunSpec,
  Snippet,
  SnippetInput,
  SnippetScope,
  Step,
  Variable,
  AmpAction,
  AmpInstance,
  Integration,
  IntegrationInput,
  IntegrationKind,
  IntegrationStatus,
  IntegrationView,
  KumaMonitor,
  PlexStream,
  Alert,
  AlertKind,
  AlertSettings,
  AlertsSnapshot,
  AlertTestResult,
  Severity,
  WebhookInfo,
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
  ImageUpdate,
  ImageStatus,
  VersionInfo,
  RemoteImage,
  DeployInput,
  Snapshot,
  SnapshotService,
  SnapshotFile,
  Stack,
  StackDir,
  MetricsEvent,
  UpdateInfo,
  Service,
  ServiceInput,
  ServiceStatus,
  ServiceEvent,
  CheckPoint,
  CheckResult,
  TlsInfo,
  HomepageCandidate,
  HomepageEntry,
  HomepagePreview,
  ImportSummary,
  Tunnel,
  TunnelInput,
  TunnelView,
  TunnelEvent,
  TunnelState,
  TunnelKind,
  TunnelTraffic,
  DisksReport,
  Disk,
  Health,
  JournalEntry,
  JournalEvent,
  JournalQuery,
  PackageManager,
  PackageUpdate,
  RunEvent,
  SmartAttr,
  SudoInput,
  SudoState,
  SystemError,
  SystemInfo,
  Unit,
  Schedules,
  SecurityReport,
  HistoryEntry,
  HistoryResult,
  PowerAction,
  WakeSent,
  Note,
  NoteInput,
  BackupJob,
  BackupJobInput,
  DeviceLabel,
  TailnetStatus,
  TailnetPeer,
  BackupStatus,
  BackupSuggestion,
  BackupTool,
  BackupSource,
  BackupResult,
  ProxyReport,
  Vm,
  VmKind,
  VmAction,
  VmReport,
  Discovery,
  Device,
  Exposure,
  ExposureKind,
  Iface,
  AddrKind,
  Seen,
  StatusPageInfo,
  StatusPageSettings,
  ProxyRoute,
  ProxyFound,
  ProxyKind,
  CertInfo,
  SecurityCheck,
  CheckStatus,
  Listener,
  ListenScope,
  Timer,
  CronJob,
  Periodic,
  Gpu,
  UnitAction,
  UpdatesReport,
  UpgradeOp,
  UpdateProgress,
  UpdateStatus,
  MountUsage,
  MetricPoint,
  PollState,
  Rate,
  Sample,
  Temp,
};

export const DEFAULT_SETTINGS: AppSettings = {
  theme: "nightgrid",
  customTheme: {},
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
  termKeyHints: true,
  termSolidBg: false,
  tailnetEnabled: false,
  tailnetAcceptRoutes: true,
  tailnetHostname: "nightgrid",
  tailnetControlUrl: "",
  metricsEnabled: true,
  metricsIntervalSecs: 2,
  metricsBackgroundSecs: 60,
  metricsRetentionDays: 7,
  servicesChecks: true,
  servicesIntervalSecs: 60,
  updateCheck: true,
  updateSkipVersion: null,
  alerts: {
    enabled: true,
    hostOffline: true,
    containerDown: true,
    diskFull: true,
    diskPercent: 90,
    serviceDown: true,
    certExpiring: true,
    certDays: 14,
    smartWarning: true,
    backupStale: true,
    logAnomaly: false,
    native: true,
    quietHours: false,
    quietStart: 22 * 60,
    quietEnd: 7 * 60,
    webhook: "none",
    webhookResolved: true,
  },
  trayEnabled: true,
  closeToTray: false,
  backupFolder: null,
  handheld: false,
  controllerNav: true,
  helperEnabled: false,
  historySearch: false,
  profileReminderDays: 7,
  lastProfileExport: null,
  profileReminderSnooze: null,
  statusPage: { enabled: false, bind: "lan", port: 8787 },
};

const MOCK_KEY = "nightgrid.mock.settings";

const mock = {
  async appInfo(): Promise<AppInfo> {
    return {
      version: "0.10.13-web",
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
  termOpenSsh: (hostId: number, cols: number, rows: number, token: string | null, edit: boolean, hints: boolean, on: TermListener): Promise<TermOpen> =>
    T() ? commands.termOpenSsh(hostId, cols, rows, token, edit, hints, channel(on)) : mockBackend.termOpenSsh(hostId, on),
  termForeground: (id: number): Promise<string | null> => (T() ? commands.termForeground(id) : mockBackend.termForeground(id)),
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
  dockerSnapshots: (hostId: number, project: string): Promise<Snapshot[]> =>
    T() ? commands.dockerSnapshots(hostId, project) : mockDocker.snapshots(hostId, project),
  dockerSnapshotTake: (hostId: number, stack: Stack): Promise<Snapshot> =>
    T() ? commands.dockerSnapshotTake(hostId, stack) : mockDocker.snapshotTake(hostId, stack),
  dockerSnapshotDelete: (id: number): Promise<null> => (T() ? commands.dockerSnapshotDelete(id) : mockDocker.snapshotDelete(id)),
  dockerStackRollback: (hostId: number, stack: Stack, snapshot: number, on: (e: ComposeEvent) => void): Promise<number> => {
    if (!T()) return mockDocker.rollback(hostId, stack, snapshot, on);
    const ch = new Channel<ComposeEvent>();
    ch.onmessage = on;
    return commands.dockerStackRollback(hostId, stack, snapshot, ch as unknown as Parameters<typeof commands.dockerStackRollback>[3]);
  },
  dockerStackCheckUpdates: (hostId: number, stack: Stack): Promise<ImageUpdate[]> =>
    T() ? commands.dockerStackCheckUpdates(hostId, stack) : mockDocker.checkUpdates(hostId, stack),
  dockerProxyRoutes: (hostId: number): Promise<ProxyReport> => (T() ? commands.dockerProxyRoutes(hostId) : mockOps.proxyRoutes(hostId)),
  dockerImageLookup: (hostId: number, image: string): Promise<RemoteImage> =>
    T() ? commands.dockerImageLookup(hostId, image) : mockDocker.imageLookup(hostId, image),
  dockerStackDeploy: (hostId: number, input: DeployInput): Promise<Stack> =>
    T() ? commands.dockerStackDeploy(hostId, input) : mockDocker.deploy(hostId, input),
  dockerComposeCancel: (run: number): Promise<null> =>
    T() ? commands.dockerComposeCancel(run) : mockDocker.composeCancel(run),
  stackDirsList: (hostId: number): Promise<StackDir[]> =>
    T() ? commands.stackDirsList(hostId) : mockDocker.stackDirs(hostId),
  stackDirsAdd: (hostId: number, path: string): Promise<StackDir> =>
    T() ? commands.stackDirsAdd(hostId, path) : mockDocker.stackDirAdd(hostId, path),
  stackDirsDelete: (id: number): Promise<null> => (T() ? commands.stackDirsDelete(id) : mockDocker.stackDirDelete(id)),
  metricsSubscribe: (on: (e: MetricsEvent) => void): Promise<null> => {
    if (!T()) return mockMetrics.subscribe(on);
    const ch = new Channel<MetricsEvent>();
    ch.onmessage = on;
    return commands.metricsSubscribe(ch as unknown as Parameters<typeof commands.metricsSubscribe>[0]);
  },
  metricsSync: (): Promise<null> => (T() ? commands.metricsSync() : mockMetrics.sync()),
  metricsSetVisible: (ids: number[]): Promise<null> => (T() ? commands.metricsSetVisible(ids) : mockMetrics.setVisible(ids)),
  metricsRetry: (hostId: number): Promise<null> => (T() ? commands.metricsRetry(hostId) : Promise.resolve(null)),
  metricsHistory: (hostId: number, from: number, to: number): Promise<MetricPoint[]> =>
    T() ? commands.metricsHistory(hostId, from, to) : mockMetrics.history(hostId, from, to),
  servicesSubscribe: (on: (e: ServiceEvent) => void): Promise<null> => {
    if (!T()) return mockServices.subscribe(on);
    const ch = new Channel<ServiceEvent>();
    ch.onmessage = on;
    return commands.servicesSubscribe(ch as unknown as Parameters<typeof commands.servicesSubscribe>[0]);
  },
  servicesCheckAll: (): Promise<null> => (T() ? commands.servicesCheckAll() : mockServices.checkAll()),
  servicesList: (): Promise<Service[]> => (T() ? commands.servicesList() : mockServices.list()),
  serviceCreate: (i: ServiceInput): Promise<Service> => (T() ? commands.serviceCreate(i) : mockServices.create(i)),
  serviceUpdate: (id: number, i: ServiceInput): Promise<Service> =>
    T() ? commands.serviceUpdate(id, i) : mockServices.update(id, i),
  serviceDelete: (id: number): Promise<null> => (T() ? commands.serviceDelete(id) : mockServices.remove(id)),
  serviceCheck: (id: number): Promise<ServiceStatus> => (T() ? commands.serviceCheck(id) : mockServices.check(id)),
  serviceHistory: (id: number, from: number): Promise<CheckPoint[]> =>
    T() ? commands.serviceHistory(id, from) : mockServices.history(id, from),
  homepageDetect: (hostId: number): Promise<HomepageCandidate[]> =>
    T() ? commands.homepageDetect(hostId) : mockServices.detect(hostId),
  homepagePreview: (side: Side, dir: string): Promise<HomepagePreview> =>
    T() ? commands.homepagePreview(side, dir) : mockServices.preview(side, dir),
  homepageImport: (side: Side, dir: string, linkHost: boolean, prune: boolean): Promise<ImportSummary> =>
    T() ? commands.homepageImport(side, dir, linkHost, prune) : mockServices.importHomepage(side, dir, linkHost, prune),
  tunnelsSubscribe: (on: (e: TunnelEvent) => void): Promise<null> => {
    if (!T()) return mockTunnels.subscribe(on);
    const ch = new Channel<TunnelEvent>();
    ch.onmessage = on;
    return commands.tunnelsSubscribe(ch as unknown as Parameters<typeof commands.tunnelsSubscribe>[0]);
  },
  tunnelsList: (): Promise<TunnelView[]> => (T() ? commands.tunnelsList() : mockTunnels.list()),
  tunnelCreate: (i: TunnelInput, startNow: boolean): Promise<Tunnel> =>
    T() ? commands.tunnelCreate(i, startNow) : mockTunnels.create(i, startNow),
  tunnelUpdate: (id: number, i: TunnelInput): Promise<Tunnel> => (T() ? commands.tunnelUpdate(id, i) : mockTunnels.update(id, i)),
  tunnelDelete: (id: number): Promise<null> => (T() ? commands.tunnelDelete(id) : mockTunnels.remove(id)),
  tunnelStart: (id: number): Promise<null> => (T() ? commands.tunnelStart(id) : mockTunnels.start(id)),
  tunnelStop: (id: number): Promise<null> => (T() ? commands.tunnelStop(id) : mockTunnels.stop(id)),
  tunnelOpenTemporary: (hostId: number, destHost: string, destPort: number): Promise<number> =>
    T() ? commands.tunnelOpenTemporary(hostId, destHost, destPort) : mockTunnels.openTemporary(hostId, destHost, destPort),
  systemInfo: (hostId: number): Promise<SystemInfo> => (T() ? commands.systemInfo(hostId) : mockSystem.info(hostId)),
  systemSudoForget: (hostId: number): Promise<null> => (T() ? commands.systemSudoForget(hostId) : mockSystem.forget(hostId)),
  systemUnits: (hostId: number): Promise<Unit[]> => (T() ? commands.systemUnits(hostId) : mockSystem.units(hostId)),
  systemSecurity: (hostId: number): Promise<SecurityReport> => (T() ? commands.systemSecurity(hostId) : mockSystem.security(hostId)),
  systemHistorySearch: (hostId: number, query: string): Promise<HistoryResult> =>
    T() ? commands.systemHistorySearch(hostId, query) : mockSystem.history(hostId, query),
  notesList: (): Promise<Note[]> => (T() ? commands.notesList() : mockNotes.list()),
  noteSave: (id: number | null, i: NoteInput): Promise<Note> => (T() ? commands.noteSave(id, i) : mockNotes.save(id, i)),
  noteDelete: (id: number): Promise<null> => (T() ? commands.noteDelete(id) : mockNotes.delete(id)),
  backupJobsList: (): Promise<BackupJob[]> => (T() ? commands.backupJobsList() : mockOps.jobsList()),
  backupJobSave: (id: number | null, i: BackupJobInput): Promise<BackupJob> => (T() ? commands.backupJobSave(id, i) : mockOps.jobSave(id, i)),
  backupJobDelete: (id: number): Promise<null> => (T() ? commands.backupJobDelete(id) : mockOps.jobDelete(id)),
  backupJobsCheck: (hostId: number): Promise<BackupStatus[]> => (T() ? commands.backupJobsCheck(hostId) : mockOps.jobsCheck(hostId)),
  backupJobsDetect: (hostId: number): Promise<BackupSuggestion[]> => (T() ? commands.backupJobsDetect(hostId) : mockOps.jobsDetect(hostId)),
  systemSchedules: (hostId: number): Promise<Schedules> => (T() ? commands.systemSchedules(hostId) : mockSystem.schedules(hostId)),
  systemUnitStatus: (hostId: number, unit: string): Promise<string> =>
    T() ? commands.systemUnitStatus(hostId, unit) : mockSystem.unitStatus(hostId, unit),
  systemUnitAction: (hostId: number, unit: string, action: UnitAction, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.systemUnitAction(hostId, unit, action, sudo) : mockSystem.unitAction(hostId, unit, action, sudo),
  systemJournalStart: (
    hostId: number,
    query: JournalQuery,
    elevated: boolean,
    sudo: SudoInput | null,
    on: (e: JournalEvent) => void,
  ): Promise<number> => {
    if (!T()) return mockSystem.journalStart(hostId, query, elevated, sudo, on);
    const ch = new Channel<JournalEvent>();
    ch.onmessage = on;
    return commands.systemJournalStart(hostId, query, elevated, sudo, ch as unknown as Parameters<typeof commands.systemJournalStart>[4]);
  },
  systemStreamStop: (run: number): Promise<null> => (T() ? commands.systemStreamStop(run) : mockSystem.streamStop(run)),
  systemUpdates: (hostId: number): Promise<UpdatesReport> => (T() ? commands.systemUpdates(hostId) : mockSystem.updates(hostId)),
  systemUpgradeRun: (hostId: number, manager: PackageManager, op: UpgradeOp, sudo: SudoInput | null, on: (e: RunEvent) => void): Promise<number> => {
    if (!T()) return mockSystem.upgradeRun(hostId, manager, op, sudo, on);
    const ch = new Channel<RunEvent>();
    ch.onmessage = on;
    return commands.systemUpgradeRun(hostId, manager, op, sudo, ch as unknown as Parameters<typeof commands.systemUpgradeRun>[4]);
  },
  systemReboot: (hostId: number, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.systemReboot(hostId, sudo) : mockSystem.reboot(hostId, sudo),
  systemPower: (hostId: number, action: PowerAction, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.systemPower(hostId, action, sudo) : mockSystem.reboot(hostId, sudo),
  hostWake: (hostId: number): Promise<WakeSent> =>
    T() ? commands.hostWake(hostId) : Promise.resolve({ via: null, mac: "00:11:22:33:44:55" }),
  systemDisks: (hostId: number, sudo: SudoInput | null): Promise<DisksReport> =>
    T() ? commands.systemDisks(hostId, sudo) : mockSystem.disks(hostId, sudo),
  systemVms: (hostId: number, elevate: boolean, sudo: SudoInput | null): Promise<VmReport> =>
    T() ? commands.systemVms(hostId, elevate, sudo) : mockOps.vms(hostId, elevate),
  systemVmAction: (hostId: number, kind: VmKind, id: string, action: VmAction, elevate: boolean, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.systemVmAction(hostId, kind, id, action, elevate, sudo) : mockOps.vmAction(kind, id, action),
  netmapDiscover: (hostId: number, sweep: boolean): Promise<Discovery> =>
    T() ? commands.netmapDiscover(hostId, sweep) : mockOps.discover(hostId, sweep),
  netmapDiscoverLocal: (sweep: boolean): Promise<Discovery> =>
    T() ? commands.netmapDiscoverLocal(sweep) : mockOps.discover(0, sweep),
  deviceLabelsList: (): Promise<DeviceLabel[]> => (T() ? commands.deviceLabelsList() : mockOps.labelsList()),
  deviceLabelSave: (l: DeviceLabel): Promise<DeviceLabel | null> => (T() ? commands.deviceLabelSave(l) : mockOps.labelSave(l)),
  tailnetStatus: (): Promise<TailnetStatus> => (T() ? commands.tailnetStatus() : mockTailnet.status()),
  tailnetSetEnabled: (on: boolean): Promise<TailnetStatus> => (T() ? commands.tailnetSetEnabled(on) : mockTailnet.setEnabled(on)),
  tailnetConfigure: (hostname: string, controlUrl: string, acceptRoutes: boolean): Promise<TailnetStatus> =>
    T() ? commands.tailnetConfigure(hostname, controlUrl, acceptRoutes) : mockTailnet.configure(acceptRoutes),
  tailnetLogin: (): Promise<null> => (T() ? commands.tailnetLogin() : mockTailnet.login()),
  tailnetLogout: (): Promise<null> => (T() ? commands.tailnetLogout() : mockTailnet.logout()),
  deviceLabelDelete: (key: string): Promise<null> => (T() ? commands.deviceLabelDelete(key) : mockOps.labelDelete(key)),
  statusPageGet: (): Promise<StatusPageInfo> => (T() ? commands.statusPageGet() : mockOps.statusGet()),
  statusPageSet: (s: StatusPageSettings): Promise<StatusPageInfo> => (T() ? commands.statusPageSet(s) : mockOps.statusSet(s)),
  statusPageNewToken: (): Promise<StatusPageInfo> => (T() ? commands.statusPageNewToken() : mockOps.statusNewToken()),
  dashboardExport: (withAddresses: boolean): Promise<string | null> =>
    T() ? commands.dashboardExport(withAddresses) : Promise.resolve("/Users/you/Downloads/nightgrid-status.html"),
  integrationsList: (): Promise<IntegrationView[]> => (T() ? commands.integrationsList() : mockIntegrations.list()),
  integrationCreate: (i: IntegrationInput, secret: string | null): Promise<IntegrationView> =>
    T() ? commands.integrationCreate(i, secret) : mockIntegrations.create(i, secret),
  integrationUpdate: (id: number, i: IntegrationInput, secret: string | null): Promise<IntegrationView> =>
    T() ? commands.integrationUpdate(id, i, secret) : mockIntegrations.update(id, i, secret),
  integrationDelete: (id: number): Promise<null> => (T() ? commands.integrationDelete(id) : mockIntegrations.remove(id)),
  integrationRevealSecret: (id: number): Promise<string | null> =>
    T() ? commands.integrationRevealSecret(id) : mockIntegrations.reveal(id),
  integrationStatus: (id: number): Promise<IntegrationStatus> =>
    T() ? commands.integrationStatus(id) : mockIntegrations.status(id),
  integrationTest: (id: number | null, i: IntegrationInput, secret: string | null): Promise<IntegrationStatus> =>
    T() ? commands.integrationTest(id, i, secret) : mockIntegrations.test(i),
  ampInstanceAction: (id: number, instance: string, action: AmpAction): Promise<null> =>
    T() ? commands.ampInstanceAction(id, instance, action) : mockIntegrations.ampAction(id, instance, action),
  snippetsList: (): Promise<Snippet[]> => (T() ? commands.snippetsList() : mockAutomation.snippets()),
  snippetSave: (id: number | null, i: SnippetInput): Promise<Snippet> =>
    T() ? commands.snippetSave(id, i) : mockAutomation.saveSnippet(id, i),
  snippetDelete: (id: number): Promise<null> => (T() ? commands.snippetDelete(id) : mockAutomation.deleteSnippet(id)),
  playbooksList: (): Promise<Playbook[]> => (T() ? commands.playbooksList() : mockAutomation.playbooks()),
  playbookSave: (id: number | null, i: PlaybookInput): Promise<Playbook> =>
    T() ? commands.playbookSave(id, i) : mockAutomation.savePlaybook(id, i),
  playbookDelete: (id: number): Promise<null> => (T() ? commands.playbookDelete(id) : mockAutomation.deletePlaybook(id)),
  /** `hostName` is only used by the browser mock. */
  automationInspect: (spec: RunSpec, values: Record<string, string>, hostId: number | null, hostName: string | null): Promise<RunInspect> =>
    T() ? commands.automationInspect(spec, values, hostId) : mockAutomation.inspect(spec, values, hostName),
  automationRun: (
    spec: RunSpec,
    hostIds: number[],
    values: Record<string, string>,
    sudoPassword: string | null,
    on: (e: AutomationEvent) => void,
  ): Promise<number> => {
    if (!T()) return mockAutomation.run(spec, hostIds, values, on);
    const ch = new Channel<AutomationEvent>();
    ch.onmessage = on;
    return commands.automationRun(spec, hostIds, values, sudoPassword, ch as unknown as Parameters<typeof commands.automationRun>[4]);
  },
  automationStop: (run: number): Promise<null> => (T() ? commands.automationStop(run) : mockAutomation.stop(run)),
  backupExport: (passphrase: string, includeSecrets: boolean, toFolder: boolean): Promise<BackupWritten | null> =>
    T() ? commands.backupExport(passphrase, includeSecrets, toFolder) : mockBackup.export(passphrase, includeSecrets, toFolder),
  backupOpen: (passphrase: string, fromFolder: boolean): Promise<BackupPreview | null> =>
    T() ? commands.backupOpen(passphrase, fromFolder) : mockBackup.open(passphrase, fromFolder),
  backupImport: (passphrase: string, mode: ImportMode): Promise<BackupImported> =>
    T() ? commands.backupImport(passphrase, mode) : mockBackup.import(passphrase, mode),
  backupPickFolder: (): Promise<BackupFolder | null> => (T() ? commands.backupPickFolder() : mockBackup.pickFolder()),
  backupFolder: (): Promise<BackupFolder | null> => (T() ? commands.backupFolder() : mockBackup.folder()),
  backupClearFolder: (): Promise<null> => (T() ? commands.backupClearFolder() : mockBackup.clearFolder()),
  helperStatus: (): Promise<HelperStatus> => (T() ? commands.helperStatus() : mockHelper.status()),
  helperDownload: (on: (p: DownloadProgress) => void): Promise<boolean> => {
    if (!T()) return mockHelper.download(on);
    const ch = new Channel<DownloadProgress>();
    ch.onmessage = on;
    return commands.helperDownload(ch as unknown as Parameters<typeof commands.helperDownload>[0]);
  },
  helperCancelDownload: (): Promise<void> => (T() ? commands.helperCancelDownload() : mockHelper.cancelDownload()),
  helperDeleteModel: (): Promise<null> => (T() ? commands.helperDeleteModel() : mockHelper.deleteModel()),
  helperUnload: (): Promise<void> => (T() ? commands.helperUnload() : mockHelper.unload()),
  helperAsk: (messages: ChatTurn[], on: (e: HelperEvent) => void): Promise<HelperReply> => {
    if (!T()) return mockHelper.ask(messages, on);
    const ch = new Channel<HelperEvent>();
    ch.onmessage = on;
    return commands.helperAsk(messages, ch as unknown as Parameters<typeof commands.helperAsk>[1]);
  },
  helperStop: (): Promise<void> => (T() ? commands.helperStop() : mockHelper.stop()),
  helperHostHistory: (hostId: number): Promise<HistoryEntry[]> =>
    T()
      ? commands.helperHostHistory(hostId)
      : Promise.resolve([
          { command: "sudo systemctl status sshd", shell: "bash", when: null },
          { command: "journalctl -b -1 -e", shell: "bash", when: null },
          { command: "sudo systemctl suspend", shell: "bash", when: null },
        ]),
  appRestart: (): Promise<void> => (T() ? commands.appRestart() : Promise.resolve(location.reload())),
  alertsSubscribe: (on: (s: AlertsSnapshot) => void): Promise<null> => {
    if (!T()) return mockAlerts.subscribe(on);
    const ch = new Channel<AlertsSnapshot>();
    ch.onmessage = on;
    return commands.alertsSubscribe(ch as unknown as Parameters<typeof commands.alertsSubscribe>[0]);
  },
  alertsRefresh: (): Promise<null> => (T() ? commands.alertsRefresh() : mockAlerts.refresh()),
  alertsClearHistory: (): Promise<null> => (T() ? commands.alertsClearHistory() : mockAlerts.clearHistory()),
  alertsWebhookGet: (): Promise<WebhookInfo> => (T() ? commands.alertsWebhookGet() : mockAlerts.webhookGet()),
  alertsWebhookSet: (url: string | null): Promise<WebhookInfo> =>
    T() ? commands.alertsWebhookSet(url) : mockAlerts.webhookSet(url),
  alertsTest: (): Promise<AlertTestResult> => (T() ? commands.alertsTest() : mockAlerts.test()),
  alertsLogSamples: (key: string): Promise<string[]> => (T() ? commands.alertsLogSamples(key) : mockOps.logSamples(key)),
  updateCheck: (): Promise<UpdateStatus> => (T() ? commands.updateCheck() : Promise.resolve({ kind: "notConfigured" })),
  updateInstall: (on: (p: UpdateProgress) => void): Promise<null> => {
    if (!T()) return Promise.reject(new Error("Updates only work in the desktop app."));
    const ch = new Channel<UpdateProgress>();
    ch.onmessage = on;
    return commands.updateInstall(ch as unknown as Parameters<typeof commands.updateInstall>[0]);
  },
  termOpenDockerExec: (hostId: number, container: string, cols: number, rows: number, on: TermListener): Promise<TermOpen> =>
    T()
      ? commands.termOpenDockerExec(hostId, container, cols, rows, channel(on))
      : mockBackend.termOpenSsh(hostId, on),
  termOpenVmConsole: (hostId: number, kind: VmKind, id: string, asRoot: boolean, cols: number, rows: number, on: TermListener): Promise<TermOpen> =>
    T()
      ? commands.termOpenVmConsole(hostId, kind, id, asRoot, cols, rows, channel(on))
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

/** The structured parts of a rejected system command, when present. */
export function systemError(e: unknown): { connect: ConnectError | null; system: SystemError | null } {
  if (e && typeof e === "object") {
    const o = e as { connect?: ConnectError | null; system?: SystemError | null };
    return { connect: o.connect ?? null, system: o.system ?? null };
  }
  return { connect: null, system: null };
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
