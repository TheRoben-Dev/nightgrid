/**
 * Backend access. Inside Tauri this calls the generated, typed commands; in a
 * plain browser (`pnpm dev` without Tauri, tests, screenshots) it falls back
 * to an in-memory/localStorage mock so the UI can be developed standalone.
 */
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import {
  type ChatTurn,
  type DownloadProgress,
  type HelperEvent,
  type HelperReply,
  type HelperStatus,
  type HelperModels,
  type ModelOption,
  commands,
  type BackupFolder_Serialize as BackupFolder,
  type BackupImported,
  type BackupPreview,
  type BackupSummary,
  type BackupWritten,
  type ImportMode,
  type Alert_Serialize as Alert,
  type AckFor,
  type MuteScope,
  type Silence_Serialize as Silence,
  type SilenceMode,
  type AccessReport_Serialize as AccessReport,
  type Crontab_Serialize as Crontab,
  type CrontabJob_Serialize as CrontabJob,
  type InstalledJob_Serialize as InstalledJob,
  type TimerJob_Serialize as TimerJob,
  type CronEdit,
  type FirewallView_Serialize as FirewallView,
  type FirewallReport_Serialize as FirewallReport,
  type FwChange_Serialize as FwChange,
  type ChangeCheck_Serialize as ChangeCheck,
  type PendingChange,
  type UfwRule_Serialize as UfwRule,
  type Zone as FirewallZone,
  type CalendarPreview,
  type EditTargets_Serialize as ProxyEditTargets,
  type CaddyTarget,
  type CaddySite,
  type CaddyEdit,
  type CaddyPreview_Serialize as CaddyPreview,
  type SaveOutcome,
  type TraefikTarget_Serialize as TraefikTarget,
  type TraefikRoute_Serialize as TraefikRoute,
  type TraefikStatic_Serialize as TraefikStatic,
  type AlertKind,
  type Dependency as AlertDependency,
  type Node as DepNode,
  type UpsCountdown,
  type BlueprintSummary_Serialize as BlueprintSummary,
  type BpInclude_Deserialize as BpInclude,
  type BpStep,
  type BpStepResult,
  type BpStepKind,
  type BpStepState,
  type StarterCatalog_Serialize as StarterCatalog,
  type StarterBlueprint_Serialize as StarterBlueprint,
  type StarterRunbook_Serialize as StarterRunbook,
  type StarterVar_Serialize as StarterVar,
  type StarterKind,
  type StarterPreset_Serialize as StarterPreset,
  type PresetKind,
  type PresetTile_Serialize as PresetTile,
  type PresetSearch_Serialize as PresetSearch,
  type PresetRoute_Serialize as PresetRoute,
  type PresetGuest_Serialize as PresetGuest,
  type AlertValue,
  type StarterTarget,
  type StarterValue,
  type ChangeArea,
  type ChangeEntry_Serialize as ChangeEntry,
  type ChangeQuery_Deserialize as ChangeQuery,
  type ChangeStats_Serialize as ChangeStats,
  type Outage as UptimeOutage,
  type UptimeDay,
  type UptimeSeries,
  type LogHit,
  type LogSearchQuery_Serialize as LogSearchQuery,
  type LogSearchResult,
  type SavedSearch_Serialize as SavedLogSearch,
  type SavedSearchInput_Serialize as SavedLogSearchInput,
  type SlaReport_Serialize as UptimeReport,
  type UpsInput_Deserialize as UpsInput,
  type UpsReading_Serialize as UpsReading,
  type UpsStatus_Serialize as UpsStatus,
  type UpsWatch_Serialize as UpsWatch,
  type AlertSettings_Serialize as AlertSettings,
  type AlertsSnapshot_Serialize as AlertsSnapshot,
  type Severity,
  type AmpAction,
  type DnsAction_Serialize as DnsAction,
  type EnvVar,
  type EnvDiff,
  type EnvVarInput_Serialize as EnvVarInput,
  type EnvDeployed_Serialize as EnvDeployed,
  type GitStack_Serialize as GitStack,
  type GitStackAdded_Serialize as GitStackAdded,
  type GitStackInput_Serialize as GitStackInput,
  type GitStatus_Serialize as GitStatus,
  type GitDeployed_Serialize as GitDeployed,
  type GitCommit,
  type NetTest_Serialize as NetTest,
  type NetTestResult_Serialize as NetTestResult,
  type CmdWidget_Serialize as CmdWidget,
  type CmdReading_Serialize as CmdReading,
  type PveOverview_Serialize as PveOverview,
  type PveTaskDetail_Serialize as PveTaskDetail,
  type PveAction,
  type PveSnapshot_Serialize as PveSnapshot,
  type PveBackups_Serialize as PveBackups,
  type PveStorageRef,
  type PveCreateInfo_Serialize as PveCreateInfo,
  type PveVmSpec_Deserialize as PveVmSpec,
  type PveCtSpec_Deserialize as PveCtSpec,
  type PveCloneSpec_Deserialize as PveCloneSpec,
  type PowerOverview_Serialize as PowerOverview,
  type HostPower_Serialize as HostPower,
  type IdleWindow,
  type QuestResult,
  type Quest_Serialize as Quest,
  type QuestState,
  type NodeView_Serialize as NodeView,
  type NodeState,
  type NodeInfoView_Serialize as NodeInfoView,
  type NodeInstallResult_Serialize as NodeInstallResult,
  type NodeSetup_Serialize as NodeSetup,
  type LinkView_Serialize as LinkView,
  type HubView_Serialize as HubView,
  type HubOverview_Serialize as HubOverview,
  type HubMemberState_Serialize as HubMemberState,
  type HubStepResult_Serialize as HubStepResult,
  type HubReport_Serialize as HubReport,
  type HubJoinResult,
  type NodeConfigFile,
  type ConfigSaveReport_Serialize as ConfigSaveReport,
  type DoctorFinding_Serialize as DoctorFinding,
  type NodePeer_Serialize as NodePeer,
  type NodeUnit,
  type NodePart,
  type InstallReport_Serialize as InstallReport,
  type SyncReport,
  type Release,
  type UpsPower_Serialize as UpsPower,
  type PowerProfile,
  type PveSandboxSpec_Deserialize as PveSandboxSpec,
  type PveSandboxDone_Serialize as PveSandboxDone,
  type PveIsolation,
  type PveBridge_Serialize as PveBridge,
  type PveAppliance,
  type PveConfig_Serialize as PveConfig,
  type PveHostDevices,
  type PveNetwork_Serialize as PveNetwork,
  type PveIfaceSpec_Deserialize as PveIfaceSpec,
  type PveNetApply_Serialize as PveNetApply,
  type PveFirewall_Serialize as PveFirewall,
  type PveFwScope,
  type PveFwRuleSpec,
  type PveFwRun,
  type PveSetting,
  type DnsStatus_Serialize as DnsStatus,
  type DnsList_Serialize as DnsList,
  type DnsRecord,
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
  type AppAction_Serialize as AppAction,
  type AppStatus_Serialize as AppStatus,
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
  type Support as PlatformSupport,
  type SupportLevel,
  type ImportResult,
  type PromptKind,
  type SshConfigCandidate_Serialize as SshConfigCandidate,
  type TermMessage_Serialize as TermMessage,
  type TermOpen,
  type LocalShell,
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
  type HealthIgnore,
  type HealthIgnoreInput_Deserialize as HealthIgnoreInput,
  type WebUiOpen_Serialize as WebUiOpen,
  type WebUiTarget_Serialize as WebUiTarget,
  type DockerError,
  type EngineInfo,
  type LogEvent_Serialize as LogEvent,
  type LogLine_Serialize as LogLine,
  type LogQuery_Deserialize as LogQuery,
  type ComposeEvent_Serialize as ComposeEvent,
  type ComposeOp,
  type ImageChange_Serialize as ImageChange,
  type ImageUpdate_Serialize as ImageUpdate,
  type UpdateHold_Serialize as UpdateHold,
  type HoldInput_Deserialize as HoldInput,
  type HoldKind,
  type HoldSupport,
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
  type MountUsage_Serialize as MountUsage,
  type Point_Serialize as MetricPoint,
  type PollState,
  type Rate,
  type Sample_Serialize as Sample,
  type Temp,
  type UpdateInfo,
  type KbPackStatus_Serialize as KbPackStatus,
  type ComponentPackInfo,
  type ComponentPackCheck_Serialize as ComponentPackCheck,
  type Broadcast_Serialize as Broadcast,
  type BroadcastStatus_Serialize as BroadcastStatus,
  type BroadcastLevel,
  type BroadcastLink,
  type KbPackContent,
  type UiPatchStatus_Serialize as UiPatchStatus,
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
  type PoolsReport_Serialize as PoolsReport,
  type Pool_Serialize as Pool,
  type PoolKind,
  type PoolDevice_Serialize as PoolDevice,
  type MountForecast_Serialize as MountForecast,
  type MountKind,
  type MountOwner,
  type GrowthReport_Serialize as GrowthReport,
  type GrowthEntry_Serialize as GrowthEntry,
  type StorageLink,
  type UsagePoint,
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
  type HostHistory_Serialize as HostHistory,
  type Lookup_Serialize as Lookup,
  type LookupResult_Serialize as LookupResult,
  type PowerAction,
  type WakeSent_Serialize as WakeSent,
  type HostSchedule_Serialize as HostSchedule,
  type HostEvent_Serialize as HostEvent,
  type DriftReport_Serialize as DriftReport,
  type FileDiff_Serialize as FileDiff,
  type DiffLine_Serialize as DiffLine,
  type PackageChange_Serialize as PackageChange,
  type FileChange,
  type SideInfo,
  type WatchItem_Serialize as WatchItem,
  type WatchInput_Deserialize as WatchInput,
  type WatchKind,
  type ExpiryPref_Serialize as ExpiryPref,
  type PrefTarget,
  type Timeline_Serialize as Timeline,
  type EventKind,
  type Level,
  type ScheduleSpec_Deserialize as ScheduleSpec,
  type ScheduledAction,
  type SshKey_Serialize as SshKey,
  type KeyList_Serialize as KeyList,
  type AuthorizedKey_Serialize as AuthorizedKey,
  type PushOutcome,
  type HistoryResult_Serialize as HistoryResult,
  type Note_Serialize as Note,
  type NoteInput_Serialize as NoteInput,
  type BackupJob,
  type DeviceLabel,
  type TailnetStatus_Serialize as TailnetStatus,
  type TailnetPeer_Serialize as TailnetPeer,
  type BackupJobInput,
  type BackupStatus_Serialize as BackupStatus,
  type LastTest,
  type RestoreTest_Serialize as RestoreTest,
  type BackupSuggestion,
  type BackupTool,
  type BackupSource,
  type BackupResult,
  type ProxyReport_Serialize as ProxyReport,
  type Vm_Serialize as Vm,
  type VmKind,
  type VmAction,
  type VmReport_Serialize as VmReport,
  type GuestAddrs_Serialize as GuestAddrs,
  type RdpTunnel,
  type VncEvent,
  type VncInput,
  type Discovery_Serialize as Discovery,
  type Device_Serialize as Device,
  type MissingDevice_Serialize as MissingDevice,
  type NetDevice_Serialize as NetDevice,
  type LayoutEntry_Serialize as LayoutEntry,
  type SavedNetwork_Serialize as SavedNetwork,
  type NetworkTag,
  type LldpNeighbour_Serialize as LldpNeighbour,
  type RangePath,
  type NetFinding_Serialize as NetFinding,
  type NetWatchRun_Serialize as NetWatchRun,
  type NetWatchSettings,
  type ToolResult_Serialize as ToolResult,
  type ToolRequest,
  type Tool as NetToolId,
  type Fact,
  type Tone,
  type ToolTable,
  type Exposure_Serialize as Exposure,
  type ExposureKind,
  type Iface,
  type AddrKind,
  type Seen,
  type StatusPageInfo_Serialize as StatusPageInfo,
  type StatusPageSettings_Serialize as StatusPageSettings,
  type SyncInfo_Serialize as SyncInfo,
  type SyncPeer_Serialize as SyncPeer,
  type SyncJoined_Serialize as SyncJoined,
  type FoundDevice,
  type SyncSnapshot_Serialize as SyncSnapshot,
  type RolledBack,
  type SnapshotReason,
  type SyncSettings_Serialize as SyncSettings,
  type SyncMode,
  type SecretCategory,
  type SyncPushed,
  type SyncRun,
  type PairingCode,
  type Merged,
  type McpSettings_Serialize as McpSettings,
  type McpStatus_Serialize as McpStatus,
  type McpCall,
  type McpAsk,
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
  type TaskAction,
  type UpdatesReport_Serialize as UpdatesReport,
  type UpgradeOp,
  type AuthNotice_Serialize as AuthNotice,
  type LogEventView_Serialize as LogEventView,
  type LogNote_Serialize as LogNote,
  type HostAuthNotice_Serialize as HostAuthNotice,
  type Notice_Serialize as Notice,
  type NoticeLevel,
  type NoticeSource,
  type NotifySettings_Serialize as NotifySettings,
  type NtfyInfo_Serialize as NtfyInfo,
  type NotifyTest_Serialize as NotifyTest,
  type PostureHost_Serialize as PostureHost,
  type PostureRun_Serialize as PostureRun,
  type PostureCheck_Serialize as PostureCheck,
  type PostureStatus,
  type SecuritySettings_Serialize as SecuritySettings,
  type SoundSettings_Serialize as SoundSettings,
  type SecretItem_Serialize as SecretItem,
  type SecretKind,
  type RotationState,
  type ManualSecret_Deserialize as ManualSecret,
  type KeyHost_Serialize as KeyHost,
  type CatalogHost_Serialize as CatalogHost,
  type CatalogEntry_Serialize as CatalogEntry,
  type CatalogMount_Serialize as CatalogMount,
  type Coverage,
  type Drill_Serialize as Drill,
  type DrillKind,
  type DrillRun_Serialize as DrillRun,
  type DrillView_Serialize as DrillView,
  type DrillBoard_Serialize as DrillBoard,
  type HostCapacity_Serialize as HostCapacity,
  type CapDisk_Serialize as CapDisk,
  type HostSpec,
  type Incident_Serialize as Incident,
  type IncidentKind,
  type Replay_Serialize as Replay,
  type ReplayEvent_Serialize as ReplayEvent,
  type ReplaySource,
} from "./bindings";
import { mockAlerts } from "./mockAlerts";
import { mockUps } from "./mockUps";
import { mockBroadcast } from "./mockBroadcast";
import { mockJournal } from "./mockJournal";
import { mockPower } from "./mockPower";
import { mockCapacity, mockCatalog, mockDrills, mockReplay } from "./mockFleet11";
import { mockSecrets } from "./mockSecrets";
import { mockPosture } from "./mockPosture";
import { mockNotify } from "./mockNotify";
import { mockHub, mockNode } from "./mockNode";
import { mockMcp } from "./mockMcp";
import { mockQuest, mockQuestLog } from "./mockQuest";
import { mockBlueprint } from "./mockBlueprint";
import { mockStarters } from "./mockStarters";
import { mockUptime } from "./mockUptime";
import { mockLogs } from "./mockLogs";
import { mockAccess } from "./mockAccess";
import { mockJobs } from "./mockJobs";
import { mockFirewall } from "./mockFirewall";
import { mockProxyEdit } from "./mockProxyEdit";
import { mockEnvVault } from "./mockEnvVault";
import { mockGitStacks } from "./mockGitStacks";
import { mockNetperf } from "./mockNetperf";
import { mockCmdWidgets } from "./mockCmdWidgets";
import { mockProxmox } from "./mockProxmox";
import { mockAutomation } from "./mockAutomation";
import { mockNotes } from "./mockNotes";
import { mockOps } from "./mockOps";
import { mockHolds } from "./mockHolds";
import { mockDesktop } from "./mockDesktop";
import { mockTailnet } from "./mockTailnet";
import { mockKeys } from "./mockKeys";
import { mockHelper } from "./mockHelper";
import { mockLookups } from "./mockLookups";
import { mockBackup } from "./mockBackup";
import { mockSync } from "./mockSync";
import { mockIntegrations } from "./mockIntegrations";
import { mockBackend } from "./mock";
import { mockDocker } from "./mockDocker";
import { mockFiles } from "./mockFiles";
import { mockMetrics } from "./mockMetrics";
import { mockServices } from "./mockServices";
import { mockTunnels } from "./mockTunnels";
import { mockSystem } from "./mockSystem";

/** Browser preview: what `sshAuthNotices` returns (tests fill it). */
export const mockAuthNotices: HostAuthNotice[] = [];

export type { AuthNotice, HostAuthNotice, LogEventView, LogNote, PlatformSupport, SupportLevel };

export type {
  ChatTurn,
  DownloadProgress,
  HelperEvent,
  HelperReply,
  HelperStatus,
  HelperModels,
  ModelOption,
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
  DnsAction,
  DnsStatus,
  EnvVar,
  EnvDiff,
  EnvVarInput,
  EnvDeployed,
  GitStack,
  GitStackAdded,
  GitStackInput,
  GitStatus,
  GitDeployed,
  GitCommit,
  NetTest,
  NetTestResult,
  CmdWidget,
  CmdReading,
  PveOverview,
  PveTaskDetail,
  PveAction,
  PveSnapshot,
  PveBackups,
  PveStorageRef,
  PveCreateInfo,
  PveVmSpec,
  PveCtSpec,
  PveCloneSpec,
  PowerOverview,
  HostPower,
  IdleWindow,
  QuestResult,
  QuestState,
  Quest,
  NodeView,
  NodeState,
  NodeInfoView,
  NodeInstallResult,
  NodeSetup,
  LinkView,
  HubView,
  HubOverview,
  HubMemberState,
  HubStepResult,
  HubReport,
  HubJoinResult,
  NodeConfigFile,
  ConfigSaveReport,
  DoctorFinding,
  NodePeer,
  NodeUnit,
  NodePart,
  InstallReport,
  SyncReport,
  Release,
  UpsPower,
  PowerProfile,
  PveSandboxSpec,
  PveSandboxDone,
  PveIsolation,
  PveBridge,
  PveAppliance,
  PveNetwork,
  PveIfaceSpec,
  PveNetApply,
  PveFirewall,
  PveFwScope,
  PveFwRuleSpec,
  PveFwRun,
  DnsList,
  DnsRecord,
  AmpInstance,
  AppAction,
  AppStatus,
  Integration,
  IntegrationInput,
  IntegrationKind,
  IntegrationStatus,
  IntegrationView,
  KumaMonitor,
  PlexStream,
  Alert,
  AlertKind,
  AlertDependency,
  DepNode,
  UpsCountdown,
  BlueprintSummary,
  BpInclude,
  BpStep,
  BpStepResult,
  BpStepKind,
  BpStepState,
  StarterCatalog,
  StarterBlueprint,
  StarterRunbook,
  StarterVar,
  StarterKind,
  StarterPreset,
  PresetKind,
  PresetTile,
  PresetSearch,
  PresetRoute,
  PresetGuest,
  AlertValue,
  StarterTarget,
  StarterValue,
  ChangeArea,
  ChangeEntry,
  ChangeQuery,
  ChangeStats,
  UptimeOutage,
  UptimeDay,
  UptimeSeries,
  UptimeReport,
  LogHit,
  LogSearchQuery,
  LogSearchResult,
  AccessReport,
  Crontab,
  CrontabJob,
  InstalledJob,
  TimerJob,
  CronEdit,
  CalendarPreview,
  FirewallView,
  FirewallReport,
  FwChange,
  ChangeCheck,
  PendingChange,
  UfwRule,
  FirewallZone,
  ProxyEditTargets,
  CaddyTarget,
  CaddySite,
  CaddyEdit,
  CaddyPreview,
  SaveOutcome,
  TraefikTarget,
  TraefikRoute,
  TraefikStatic,
  SavedLogSearch,
  SavedLogSearchInput,
  UpsInput,
  UpsReading,
  UpsStatus,
  UpsWatch,
  AlertSettings,
  AlertsSnapshot,
  AckFor,
  MuteScope,
  Silence,
  SilenceMode,
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
  LocalShell,
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
  HealthIgnore,
  HealthIgnoreInput,
  WebUiOpen,
  WebUiTarget,
  DockerError,
  EngineInfo,
  LogEvent,
  LogLine,
  LogQuery,
  ComposeEvent,
  ComposeOp,
  ImageChange,
  ImageUpdate,
  UpdateHold,
  HoldInput,
  HoldKind,
  HoldSupport,
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
  KbPackStatus,
  KbPackContent,
  ComponentPackInfo,
  ComponentPackCheck,
  Broadcast,
  BroadcastStatus,
  BroadcastLevel,
  BroadcastLink,
  UiPatchStatus,
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
  PoolsReport,
  Pool,
  PoolKind,
  PoolDevice,
  MountForecast,
  MountKind,
  MountOwner,
  GrowthReport,
  GrowthEntry,
  StorageLink,
  UsagePoint,
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
  HostHistory,
  HistoryResult,
  PowerAction,
  WakeSent,
  HostSchedule,
  HostEvent,
  DriftReport,
  FileDiff,
  DiffLine,
  PackageChange,
  FileChange,
  SideInfo,
  WatchItem,
  WatchInput,
  WatchKind,
  ExpiryPref,
  PrefTarget,
  Timeline,
  EventKind,
  Level,
  ScheduleSpec,
  ScheduledAction,
  SshKey,
  KeyList,
  AuthorizedKey,
  PushOutcome,
  Note,
  NoteInput,
  BackupJob,
  BackupJobInput,
  DeviceLabel,
  TailnetStatus,
  TailnetPeer,
  BackupStatus,
  LastTest,
  RestoreTest,
  BackupSuggestion,
  BackupTool,
  BackupSource,
  BackupResult,
  ProxyReport,
  Vm,
  VmKind,
  VmAction,
  VmReport,
  GuestAddrs,
  RdpTunnel,
  VncEvent,
  VncInput,
  Discovery,
  Device,
  MissingDevice,
  NetDevice,
  LayoutEntry,
  SavedNetwork,
  NetworkTag,
  LldpNeighbour,
  RangePath,
  NetFinding,
  NetWatchRun,
  NetWatchSettings,
  ToolResult,
  ToolRequest,
  NetToolId,
  Fact,
  Tone,
  ToolTable,
  Exposure,
  ExposureKind,
  Iface,
  AddrKind,
  Seen,
  StatusPageInfo,
  StatusPageSettings,
  McpSettings,
  SyncInfo,
  SyncPeer,
  SyncJoined,
  FoundDevice,
  SyncSnapshot,
  RolledBack,
  SnapshotReason,
  SyncSettings,
  SyncMode,
  SecretCategory,
  SyncPushed,
  SyncRun,
  PairingCode,
  Merged,
  McpStatus,
  McpCall,
  McpAsk,
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
  TaskAction,
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
  Notice,
  NoticeLevel,
  NoticeSource,
  NotifySettings,
  NtfyInfo,
  NotifyTest,
  PostureHost,
  PostureRun,
  PostureCheck,
  PostureStatus,
  SecuritySettings,
  SoundSettings,
  SecretItem,
  SecretKind,
  RotationState,
  ManualSecret,
  KeyHost,
  CatalogHost,
  CatalogEntry,
  CatalogMount,
  Coverage,
  Drill,
  DrillKind,
  DrillRun,
  DrillView,
  DrillBoard,
  HostCapacity,
  CapDisk,
  HostSpec,
  Incident,
  IncidentKind,
  Replay,
  ReplayEvent,
  ReplaySource,
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
  termLocalShell: "",
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
  kbUpdates: true,
  uiPatches: true,
  broadcasts: true,
  dismissedBroadcasts: [],
  lastSeenVersion: null,
  alerts: {
    enabled: true,
    hostOffline: true,
    containerDown: true,
    diskFull: true,
    diskPercent: 90,
    diskSkipSystem: true,
    serviceDown: true,
    certExpiring: true,
    certDays: 14,
    smartWarning: true,
    backupStale: true,
    diskForecast: true,
    forecastDays: 14,
    poolDegraded: true,
    logAnomaly: false,
    logRepeatCount: 3,
    logRepeatMinutes: 10,
    logGraceMinutes: 5,
    logNotFoundNoise: true,
    logMutes: [],
    foldDependent: true,
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
  swipeNav: true,
  helperEnabled: false,
  helperModel: "qwen3-4b-instruct-2507-q4km",
  historySearch: false,
  profileReminderDays: 7,
  lastProfileExport: null,
  dashboards: [],
  dashboardActive: 0,
  dashboardPresets: [],
  tourOffered: false,
  disabledFeatures: [],
  installedComponents: null,
  profileReminderSnooze: null,
  statusPage: { enabled: false, bind: "lan", port: 8787 },
  sync: { enabled: false, port: 47920, deviceName: "", discoverable: false, passphrasePairing: false, keepSnapshots: 3 },
  mcp: { enabled: false, port: 47821, allowWrites: false },
  power: { pricePerKwh: 0, currency: "" },
  notify: { native: true, ntfy: false, ntfyLevel: "warn", ntfyOk: true, muted: [] },
  security: { postureDaily: true, rotationNotices: true },
  sound: { volume: 40, muted: ["navigation"], quietHours: false, quietStart: 1320, quietEnd: 420, unfocusedMute: true },
  netWatch: { enabled: false, scanner: null, minutes: 60, sweep: false, alertNew: true, alertMissing: true, alertRisks: true },
};

const MOCK_KEY = "nightgrid.mock.settings";

const mock = {
  async appInfo(): Promise<AppInfo> {
    return {
      version: "0.10.63-web",
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
  return ch as unknown as Parameters<typeof commands.termOpenLocal>[3];
}

const T = inTauri;

/** Outside the desktop app there is no downloaded Library content. */
/** Browser preview: packs from localStorage `nightgrid.mockComponentPacks` (for trying the loader). */
function mockComponentPacks(): ComponentPackInfo[] {
  try {
    const v = JSON.parse(localStorage.getItem("nightgrid.mockComponentPacks") ?? "[]") as unknown;
    return Array.isArray(v) ? (v as ComponentPackInfo[]) : [];
  } catch {
    return [];
  }
}
const NO_KB_PACK: KbPackStatus = { installed: null, checkedAt: null, error: null, updated: false, notConfigured: true };
/** Nor interface patches. */
const NO_UI_PATCH: UiPatchStatus = { running: null, waiting: null, bundledBuilt: 0, skipped: null, checkedAt: null, error: null, updated: false, notConfigured: true };

function jobsChannel(listener: (jobs: JobView[]) => void) {
  const ch = new Channel<JobView[]>();
  ch.onmessage = listener;
  return ch as unknown as Parameters<typeof commands.transfersSubscribe>[0];
}


export const api = {
  appInfo: (): Promise<AppInfo> => (T() ? commands.appInfo() : mock.appInfo()),
  /** Saves a diagnostics report where the user picks; the path, or null if cancelled. */
  diagnosticsExport: (): Promise<string | null> => (T() ? commands.diagnosticsExport() : Promise.resolve(null)),
  settingsGet: (): Promise<AppSettings> => (T() ? commands.settingsGet() : mock.settingsGet()),
  settingsSet: (s: AppSettings): Promise<AppSettings> => (T() ? commands.settingsSet(s) : mock.settingsSet(s)),

  hostsList: (): Promise<Host[]> => (T() ? commands.hostsList() : mockBackend.hostsList()),
  /** What works on Windows hosts (the browser preview has no Windows hosts). */
  platformWindowsSupport: (): Promise<PlatformSupport[]> => (T() ? commands.platformWindowsSupport() : Promise.resolve([])),
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

  /** `shell`: an id from `termLocalShells` (Windows only; null = the default shell). */
  termOpenLocal: (cols: number, rows: number, shell: string | null, on: TermListener): Promise<number> =>
    T() ? commands.termOpenLocal(cols, rows, shell, channel(on)) : mockBackend.termOpenLocal(on),
  /** Windows: PowerShell, cmd and WSL distros. Empty on macOS/Linux and in the browser preview. */
  termLocalShells: (): Promise<LocalShell[]> => (T() ? commands.termLocalShells() : Promise.resolve([])),
  termOpenSsh: (hostId: number, cols: number, rows: number, token: string | null, edit: boolean, hints: boolean, on: TermListener): Promise<TermOpen> =>
    T() ? commands.termOpenSsh(hostId, cols, rows, token, edit, hints, channel(on)) : mockBackend.termOpenSsh(hostId, on),
  /** Login notices (a Tailscale SSH approval link) for hosts still connecting. */
  sshAuthNotices: (): Promise<HostAuthNotice[]> => (T() ? commands.sshAuthNotices() : Promise.resolve(mockAuthNotices.slice())),
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
  dockerFixAccess: (hostId: number, steps: string[], relogin: boolean, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.dockerFixAccess(hostId, steps, relogin, sudo) : mockDocker.fixAccess(hostId),
  dockerSetUser: (hostId: number, user: string | null): Promise<null> =>
    T() ? commands.dockerSetUser(hostId, user) : mockBackend.hostSetDockerUser(hostId, user).then(() => mockDocker.setUser(hostId, user)),
  dockerHealthIgnores: (): Promise<HealthIgnore[]> => (T() ? commands.dockerHealthIgnores() : mockDocker.healthIgnores()),
  dockerSetHealthIgnored: (input: HealthIgnoreInput, ignored: boolean): Promise<HealthIgnore | null> =>
    T() ? commands.dockerSetHealthIgnored(input, ignored) : mockDocker.setHealthIgnored(input, ignored),
  dockerHealthIgnoreDelete: (id: number): Promise<null> => (T() ? commands.dockerHealthIgnoreDelete(id) : mockDocker.deleteHealthIgnore(id)),
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
  /** Opens a web UI in its own Nightgrid window (D-147); in the browser build, a new tab. */
  webuiOpen: (hostId: number | null, url: string, title: string): Promise<WebUiOpen> =>
    T()
      ? commands.webuiOpen(hostId, url, title)
      : Promise.resolve().then(() => {
          window.open(url, "_blank", "noopener");
          return { opened: true, url, viaTunnel: false, problem: null };
        }),
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
  backupTestsList: (): Promise<LastTest[]> => (T() ? commands.backupTestsList() : mockOps.testsList()),
  backupTestRestore: (jobId: number, sudo: SudoInput | null): Promise<RestoreTest> =>
    T() ? commands.backupTestRestore(jobId, sudo) : mockOps.testRestore(jobId, sudo),
  backupJobsDetect: (hostId: number): Promise<BackupSuggestion[]> => (T() ? commands.backupJobsDetect(hostId) : mockOps.jobsDetect(hostId)),
  systemSchedules: (hostId: number): Promise<Schedules> => (T() ? commands.systemSchedules(hostId) : mockSystem.schedules(hostId)),
  systemTaskAction: (hostId: number, task: string, action: TaskAction): Promise<null> =>
    T() ? commands.systemTaskAction(hostId, task, action) : Promise.resolve(null),
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
  /** Holds or releases packages on the host itself (apt-mark, dnf versionlock, hidden Windows updates). */
  systemPackageHold: (hostId: number, manager: PackageManager, names: string[], hold: boolean, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.systemPackageHold(hostId, manager, names, hold, sudo) : mockHolds.packageHold(hostId, manager, names, hold),
  updateHoldsList: (): Promise<UpdateHold[]> => (T() ? commands.updateHoldsList() : mockHolds.list()),
  updateHoldSave: (input: HoldInput): Promise<UpdateHold> => (T() ? commands.updateHoldSave(input) : mockHolds.save(input)),
  updateHoldDelete: (id: number): Promise<null> => (T() ? commands.updateHoldDelete(id) : mockHolds.remove(id)),
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
  powerSchedules: (hostId: number): Promise<HostSchedule[]> => (T() ? commands.powerSchedules(hostId) : mockOps.powerSchedules(hostId)),
  powerScheduleSet: (hostId: number, spec: ScheduleSpec, sudo: SudoInput | null): Promise<HostSchedule> =>
    T() ? commands.powerScheduleSet(hostId, spec, sudo) : mockOps.powerScheduleSet(hostId, spec),
  powerScheduleRemove: (onHostId: number, unit: string, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.powerScheduleRemove(onHostId, unit, sudo) : mockOps.powerScheduleRemove(unit),
  timelineGet: (hostId: number, days: number): Promise<Timeline> => (T() ? commands.timelineGet(hostId, days) : mockOps.timeline(hostId, days)),
  timelineClear: (hostId: number): Promise<null> => (T() ? commands.timelineClear(hostId) : mockOps.timelineClear(hostId)),
  watchList: (): Promise<WatchItem[]> => (T() ? commands.watchList() : mockOps.watchList()),
  watchSave: (input: WatchInput): Promise<WatchItem> => (T() ? commands.watchSave(input) : mockOps.watchSave(input)),
  watchDelete: (id: number): Promise<null> => (T() ? commands.watchDelete(id) : mockOps.watchDelete(id)),
  watchCheck: (): Promise<WatchItem[]> => (T() ? commands.watchCheck() : mockOps.watchList()),
  watchLookup: (id: number, via: number): Promise<WatchItem> => (T() ? commands.watchLookup(id, via) : mockOps.watchLookup(id)),
  expiryPrefs: (): Promise<ExpiryPref[]> => (T() ? commands.expiryPrefs() : mockOps.expiryPrefs()),
  expiryPrefSave: (pref: ExpiryPref): Promise<ExpiryPref[]> => (T() ? commands.expiryPrefSave(pref) : mockOps.expiryPrefSave(pref)),
  confdiffBaseline: (hostId: number): Promise<SideInfo | null> => (T() ? commands.confdiffBaseline(hostId) : mockOps.confdiffBaseline(hostId)),
  confdiffSaveBaseline: (hostId: number): Promise<SideInfo> => (T() ? commands.confdiffSaveBaseline(hostId) : mockOps.confdiffSaveBaseline(hostId)),
  confdiffCompare: (hostId: number, other: number | null, hideHostSpecific: boolean): Promise<DriftReport> =>
    T() ? commands.confdiffCompare(hostId, other, hideHostSpecific) : mockOps.confdiffCompare(hostId, other, hideHostSpecific),
  confdiffFile: (hostId: number, other: number | null, path: string): Promise<FileDiff> =>
    T() ? commands.confdiffFile(hostId, other, path) : mockOps.confdiffFile(path),
  keysList: (): Promise<KeyList> => (T() ? commands.keysList() : mockKeys.list()),
  keysGenerate: (name: string, comment: string, passphrase: string | null): Promise<SshKey> =>
    T() ? commands.keysGenerate(name, comment, passphrase) : mockKeys.generate(name, comment, passphrase),
  keysAuthorized: (hostId: number): Promise<AuthorizedKey[]> => (T() ? commands.keysAuthorized(hostId) : mockKeys.authorized(hostId)),
  keysPush: (hostId: number, fingerprint: string): Promise<PushOutcome> =>
    T() ? commands.keysPush(hostId, fingerprint) : mockKeys.push(hostId, fingerprint),
  keysRemove: (hostId: number, fingerprint: string): Promise<number> =>
    T() ? commands.keysRemove(hostId, fingerprint) : mockKeys.remove(hostId, fingerprint),
  keysVerify: (hostId: number, file: string, passphrase: string | null): Promise<null> =>
    T() ? commands.keysVerify(hostId, file, passphrase) : mockKeys.verify(),
  systemPools: (hostId: number): Promise<PoolsReport> => (T() ? commands.systemPools(hostId) : mockSystem.pools(hostId)),
  diskForecasts: (hostId: number): Promise<MountForecast[]> => (T() ? commands.diskForecasts(hostId) : mockSystem.forecasts(hostId)),
  diskGrowth: (hostId: number, path: string, elevate: boolean, sudo: SudoInput | null): Promise<GrowthReport> =>
    T() ? commands.diskGrowth(hostId, path, elevate, sudo) : mockSystem.growth(hostId, path),
  storageLinks: (): Promise<StorageLink[]> => (T() ? commands.storageLinks() : mockSystem.links()),
  storageLinkAdd: (hostId: number, ownerHostId: number, ownerMount: string): Promise<StorageLink> =>
    T() ? commands.storageLinkAdd(hostId, ownerHostId, ownerMount) : mockSystem.linkAdd(hostId, ownerHostId, ownerMount),
  storageLinkDelete: (id: number): Promise<null> => (T() ? commands.storageLinkDelete(id) : mockSystem.linkDelete(id)),
  systemDisks: (hostId: number, sudo: SudoInput | null): Promise<DisksReport> =>
    T() ? commands.systemDisks(hostId, sudo) : mockSystem.disks(hostId, sudo),
  systemVms: (hostId: number, elevate: boolean, sudo: SudoInput | null): Promise<VmReport> =>
    T() ? commands.systemVms(hostId, elevate, sudo) : mockOps.vms(hostId, elevate),
  systemVmAction: (hostId: number, kind: VmKind, id: string, action: VmAction, elevate: boolean, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.systemVmAction(hostId, kind, id, action, elevate, sudo) : mockOps.vmAction(kind, id, action),
  desktopOpen: (hostId: number, kind: VmKind, id: string, elevate: boolean, sudo: SudoInput | null, on: (e: VncEvent) => void): Promise<number> => {
    if (!T()) return mockDesktop.open(kind, id, on);
    const ch = new Channel<VncEvent>();
    ch.onmessage = on;
    return commands.desktopOpen(hostId, kind, id, elevate, sudo, ch as unknown as Parameters<typeof commands.desktopOpen>[5]);
  },
  /** The next patch of changed pixels (raw bytes; empty when nothing changed within a second). */
  desktopFrame: (session: number): Promise<ArrayBuffer> => (T() ? invoke<ArrayBuffer>("desktop_frame", { session }) : mockDesktop.frame(session)),
  desktopInput: (session: number, events: VncInput[]): Promise<null> => (T() ? commands.desktopInput(session, events) : mockDesktop.input(session, events)),
  desktopClose: (session: number): Promise<void> => (T() ? commands.desktopClose(session) : Promise.resolve(mockDesktop.close(session))),
  desktopGuestAddrs: (hostId: number, kind: VmKind, id: string, elevate: boolean, sudo: SudoInput | null): Promise<GuestAddrs> =>
    T() ? commands.desktopGuestAddrs(hostId, kind, id, elevate, sudo) : mockDesktop.guestAddrs(kind, id),
  desktopRdpOpen: (hostId: number, guest: string, addr: string, port: number | null): Promise<RdpTunnel> =>
    T() ? commands.desktopRdpOpen(hostId, guest, addr, port) : mockDesktop.rdpOpen(hostId, guest, addr),
  desktopRdpList: (): Promise<RdpTunnel[]> => (T() ? commands.desktopRdpList() : mockDesktop.rdpList()),
  desktopRdpClose: (id: number): Promise<void> => (T() ? commands.desktopRdpClose(id) : Promise.resolve(mockDesktop.rdpClose(id))),
  netmapDiscover: (hostId: number, sweep: boolean, ranges: string[] = []): Promise<Discovery> =>
    T() ? commands.netmapDiscover(hostId, sweep, ranges) : mockOps.discover(hostId, sweep, ranges),
  netmapDiscoverLocal: (sweep: boolean, ranges: string[] = []): Promise<Discovery> =>
    T() ? commands.netmapDiscoverLocal(sweep, ranges) : mockOps.discover(0, sweep, ranges),
  netmapInventory: (): Promise<NetDevice[]> => (T() ? commands.netmapInventory() : mockOps.inventory()),
  netmapDeviceTrust: (key: string, trusted: boolean): Promise<null> => (T() ? commands.netmapDeviceTrust(key, trusted) : mockOps.trust(key, trusted)),
  netmapDeviceForget: (key: string): Promise<null> => (T() ? commands.netmapDeviceForget(key) : mockOps.forget(key)),
  netmapNetworks: (): Promise<SavedNetwork[]> => (T() ? commands.netmapNetworks() : mockOps.networks()),
  netmapNetworkScan: (id: number): Promise<Discovery | null> => (T() ? commands.netmapNetworkScan(id) : mockOps.networkScan(id)),
  netmapNetworkRename: (id: number, name: string): Promise<SavedNetwork> => (T() ? commands.netmapNetworkRename(id, name) : mockOps.networkRename(id, name)),
  netmapNetworkForget: (id: number): Promise<null> => (T() ? commands.netmapNetworkForget(id) : mockOps.networkForget(id)),
  netmapNetworkMerge: (from: number, into: number): Promise<SavedNetwork> => (T() ? commands.netmapNetworkMerge(from, into) : mockOps.networkMerge(from, into)),
  netmapLayoutGet: (): Promise<LayoutEntry[]> => (T() ? commands.netmapLayoutGet() : mockOps.layoutGet()),
  netmapLayoutSave: (entries: LayoutEntry[], replace = false): Promise<null> =>
    T() ? commands.netmapLayoutSave(entries, replace) : mockOps.layoutSave(entries, replace),
  netTool: (hostId: number | null, req: ToolRequest): Promise<ToolResult> => (T() ? commands.netTool(hostId, req) : mockOps.tool(hostId, req)),
  netmapSecurity: (hostId: number | null, ips: string[]): Promise<NetFinding[]> =>
    T() ? commands.netmapSecurity(hostId, ips) : mockOps.security(ips),
  netmapWatchLast: (): Promise<NetWatchRun | null> => (T() ? commands.netmapWatchLast() : Promise.resolve(null)),
  netMacVendor: (mac: string): Promise<string | null> => (T() ? commands.netMacVendor(mac) : mockOps.macVendor(mac)),
  netWake: (hostId: number | null, mac: string): Promise<string> => (T() ? commands.netWake(hostId, mac) : mockOps.wake(mac)),
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
  syncInfo: (): Promise<SyncInfo> => (T() ? commands.syncInfo() : mockSync.info()),
  syncConfigure: (s: SyncSettings): Promise<SyncInfo> => (T() ? commands.syncConfigure(s) : mockSync.configure(s)),
  syncPairStart: (acceptChanges: boolean): Promise<SyncInfo> => (T() ? commands.syncPairStart(acceptChanges) : mockSync.pairStart(acceptChanges)),
  syncPairCancel: (): Promise<SyncInfo> => (T() ? commands.syncPairCancel() : mockSync.pairCancel()),
  syncPairJoin: (addr: string, code: string, acceptChanges: boolean): Promise<SyncPeer> =>
    T() ? commands.syncPairJoin(addr, code, acceptChanges) : mockSync.pairJoin(addr, code, acceptChanges),
  syncDiscover: (): Promise<FoundDevice[]> => (T() ? commands.syncDiscover() : mockSync.discover()),
  syncAskCode: (addr: string, port: number): Promise<null> => (T() ? commands.syncAskCode(addr, port) : mockSync.askCode(addr)),
  syncJoin: (
    addr: string,
    port: number,
    code: string | null,
    passphrase: string | null,
    salt: string | null,
    importMode: ImportMode | null,
    twoWay: boolean,
    secrets: SecretCategory[],
  ): Promise<SyncJoined> =>
    T() ? commands.syncJoin(addr, port, code, passphrase, salt, importMode, twoWay, secrets) : mockSync.join(addr, code, passphrase, importMode, twoWay, secrets),
  syncPassphrase: (passphrase: string | null): Promise<SyncInfo> => (T() ? commands.syncPassphrase(passphrase) : mockSync.passphrase(passphrase)),
  syncSnapshots: (): Promise<SyncSnapshot[]> => (T() ? commands.syncSnapshots() : mockSync.snapshots()),
  syncSnapshotTake: (): Promise<SyncSnapshot[]> => (T() ? commands.syncSnapshotTake() : mockSync.snapshotTake()),
  syncRollback: (id: number): Promise<RolledBack> => (T() ? commands.syncRollback(id) : mockSync.rollback(id)),
  syncPeerSet: (id: string, name: string, mode: SyncMode, secrets: SecretCategory[], acceptChanges: boolean): Promise<SyncPeer> =>
    T() ? commands.syncPeerSet(id, name, mode, secrets, acceptChanges) : mockSync.peerSet(id, name, mode, secrets, acceptChanges),
  syncUnpair: (id: string): Promise<null> => (T() ? commands.syncUnpair(id) : mockSync.unpair(id)),
  syncCheck: (id: string): Promise<SyncPeer> => (T() ? commands.syncCheck(id) : mockSync.check(id)),
  syncPush: (id: string, mode: ImportMode): Promise<SyncPushed> => (T() ? commands.syncPush(id, mode) : mockSync.push(id, mode)),
  syncPull: (id: string, mode: ImportMode): Promise<BackupImported> => (T() ? commands.syncPull(id, mode) : mockSync.pull(id, mode)),
  syncNow: (id: string): Promise<SyncRun> => (T() ? commands.syncNow(id) : mockSync.now(id)),
  mcpStatus: (): Promise<McpStatus> => (T() ? commands.mcpStatus() : mockMcp.status()),
  mcpSet: (s: McpSettings): Promise<McpStatus> => (T() ? commands.mcpSet(s) : mockMcp.set(s)),
  mcpNewToken: (): Promise<McpStatus> => (T() ? commands.mcpNewToken() : mockMcp.newToken()),
  mcpAnswer: (id: number, allow: boolean): Promise<null> => (T() ? commands.mcpAnswer(id, allow) : mockMcp.answer(id, allow)),
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
  dnsFilterAction: (id: number, action: DnsAction): Promise<null> => (T() ? commands.dnsFilterAction(id, action) : mockIntegrations.dnsAction(id, action)),
  synologyTrustDevice: (id: number, code: string): Promise<boolean> =>
    T() ? commands.synologyTrustDevice(id, code) : mockIntegrations.trustDevice(id, code),
  synologyForgetDevice: (id: number): Promise<null> => (T() ? commands.synologyForgetDevice(id) : mockIntegrations.forgetDevice(id)),
  integrationAction: (id: number, target: string, action: string): Promise<null> =>
    T() ? commands.integrationAction(id, target, action) : mockIntegrations.action(id, target, action),
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
  helperStatus: (): Promise<HelperStatus> => (T() ? commands.helperStatus() : mock.settingsGet().then((s) => (mockHelper.setActive(s.helperModel), mockHelper.status()))),
  helperModels: (): Promise<HelperModels> => (T() ? commands.helperModels() : mock.settingsGet().then((s) => (mockHelper.setActive(s.helperModel), mockHelper.models()))),
  helperDownload: (modelId: string, on: (p: DownloadProgress) => void): Promise<boolean> => {
    if (!T()) return mockHelper.download(modelId, on);
    const ch = new Channel<DownloadProgress>();
    ch.onmessage = on;
    return commands.helperDownload(modelId, ch as unknown as Parameters<typeof commands.helperDownload>[1]);
  },
  helperCancelDownload: (): Promise<void> => (T() ? commands.helperCancelDownload() : mockHelper.cancelDownload()),
  helperDeleteModel: (modelId: string): Promise<null> => (T() ? commands.helperDeleteModel(modelId) : mockHelper.deleteModel(modelId)),
  helperUnload: (): Promise<void> => (T() ? commands.helperUnload() : mockHelper.unload()),
  helperAsk: (messages: ChatTurn[], on: (e: HelperEvent) => void): Promise<HelperReply> => {
    if (!T()) return mockHelper.ask(messages, on);
    const ch = new Channel<HelperEvent>();
    ch.onmessage = on;
    return commands.helperAsk(messages, ch as unknown as Parameters<typeof commands.helperAsk>[1]);
  },
  helperStop: (): Promise<void> => (T() ? commands.helperStop() : mockHelper.stop()),
  helperHostHistory: (hostId: number, terms: string[] = []): Promise<HostHistory> =>
    T()
      ? commands.helperHostHistory(hostId, terms)
      : Promise.resolve({
          recent: [
            { command: "sudo systemctl status sshd", shell: "bash", when: null },
            { command: "journalctl -b -1 -e", shell: "bash", when: null },
            { command: "sudo systemctl suspend", shell: "bash", when: null },
          ],
          matches: terms.some((t) => "nginx".includes(t.toLowerCase())) ? [{ command: "sudo nginx -t && sudo systemctl reload nginx", shell: "bash", when: null }] : [],
        }),
  helperHostLookups: (hostId: number, lookups: Lookup[], terms: string[] = []): Promise<LookupResult[]> =>
    T() ? commands.helperHostLookups(hostId, lookups, terms) : mockLookups(lookups, terms),
  appRestart: (): Promise<void> => (T() ? commands.appRestart() : Promise.resolve(location.reload())),
  alertsSubscribe: (on: (s: AlertsSnapshot) => void): Promise<null> => {
    if (!T()) return mockAlerts.subscribe(on);
    const ch = new Channel<AlertsSnapshot>();
    ch.onmessage = on;
    return commands.alertsSubscribe(ch as unknown as Parameters<typeof commands.alertsSubscribe>[0]);
  },
  alertsRefresh: (): Promise<null> => (T() ? commands.alertsRefresh() : mockAlerts.refresh()),
  alertsClearHistory: (): Promise<null> => (T() ? commands.alertsClearHistory() : mockAlerts.clearHistory()),
  alertsAck: (key: string, how: AckFor): Promise<Silence> => (T() ? commands.alertsAck(key, how) : mockAlerts.ack(key, how)),
  alertsMute: (key: string, scope: MuteScope, secs: number | null): Promise<Silence> =>
    T() ? commands.alertsMute(key, scope, secs) : mockAlerts.mute(key, scope, secs),
  alertsUnsilence: (id: number): Promise<null> => (T() ? commands.alertsUnsilence(id) : mockAlerts.unsilence(id)),
  alertsWebhookGet: (): Promise<WebhookInfo> => (T() ? commands.alertsWebhookGet() : mockAlerts.webhookGet()),
  alertsWebhookSet: (url: string | null): Promise<WebhookInfo> =>
    T() ? commands.alertsWebhookSet(url) : mockAlerts.webhookSet(url),
  alertsTest: (): Promise<AlertTestResult> => (T() ? commands.alertsTest() : mockAlerts.test()),
  alertsDepsList: (): Promise<AlertDependency[]> => (T() ? commands.alertsDepsList() : mockAlerts.depsList()),
  alertsDepsAdd: (child: DepNode, parent: DepNode): Promise<AlertDependency> =>
    T() ? commands.alertsDepsAdd(child, parent) : mockAlerts.depsAdd(child, parent),
  alertsDepsDelete: (id: number): Promise<null> => (T() ? commands.alertsDepsDelete(id) : mockAlerts.depsDelete(id)),
  upsList: (): Promise<UpsWatch[]> => (T() ? commands.upsList() : mockUps.list()),
  upsSave: (input: UpsInput): Promise<UpsWatch> => (T() ? commands.upsSave(input) : mockUps.save(input)),
  upsDelete: (id: number): Promise<null> => (T() ? commands.upsDelete(id) : mockUps.delete(id)),
  upsStatus: (): Promise<UpsStatus[]> => (T() ? commands.upsStatus() : mockUps.status()),
  upsReadNow: (hostId: number, target: string): Promise<UpsReading> => (T() ? commands.upsReadNow(hostId, target) : mockUps.readNow(hostId, target)),
  upsCancel: (): Promise<null> => (T() ? commands.upsCancel() : mockUps.cancel()),
  blueprintCapture: (hostId: number, include: BpInclude, sudo: SudoInput | null): Promise<BlueprintSummary> =>
    T() ? commands.blueprintCapture(hostId, include, sudo) : mockBlueprint.capture(hostId, include),
  blueprintSave: (hostId: number, passphrase: string): Promise<string | null> => (T() ? commands.blueprintSave(hostId, passphrase) : mockBlueprint.save()),
  blueprintOpen: (passphrase: string): Promise<BlueprintSummary | null> => (T() ? commands.blueprintOpen(passphrase) : mockBlueprint.open(passphrase)),
  blueprintClose: (): Promise<null> => (T() ? commands.blueprintClose().then(() => null) : mockBlueprint.close()),
  blueprintPlan: (hostId: number, sudo: SudoInput | null): Promise<BpStep[]> => (T() ? commands.blueprintPlan(hostId, sudo) : mockBlueprint.plan()),
  blueprintApply: (hostId: number, steps: string[], sudo: SudoInput | null): Promise<BpStepResult[]> =>
    T() ? commands.blueprintApply(hostId, steps, sudo) : mockBlueprint.apply(hostId, steps),
  startersList: (): Promise<StarterCatalog> => (T() ? commands.startersList() : mockStarters.list()),
  starterTarget: (hostId: number): Promise<StarterTarget> => (T() ? commands.starterTarget(hostId) : mockStarters.target()),
  starterOpen: (id: string, values: StarterValue[], parent: string): Promise<BlueprintSummary> =>
    T() ? commands.starterOpen(id, values, parent) : mockStarters.open(id, values, parent).then((s) => (mockBlueprint.markOpened(), s)),
  journalQuery: (query: ChangeQuery): Promise<ChangeEntry[]> => (T() ? commands.journalQuery(query) : mockJournal.query(query)),
  journalStats: (): Promise<ChangeStats> => (T() ? commands.journalStats() : mockJournal.stats()),
  journalNote: (hostId: number | null, title: string, detail: string, at: number | null): Promise<ChangeEntry> =>
    T() ? commands.journalNote(hostId, title, detail, at) : mockJournal.note(hostId, title, detail, at),
  journalEdit: (id: number, title: string, detail: string): Promise<null> => (T() ? commands.journalEdit(id, title, detail) : mockJournal.edit(id, title, detail)),
  journalDelete: (id: number): Promise<null> => (T() ? commands.journalDelete(id) : mockJournal.delete(id)),
  journalClear: (): Promise<number> => (T() ? commands.journalClear() : mockJournal.clear()),
  journalForDrift: (hostIds: number[], since: number, max: number): Promise<string> =>
    T() ? commands.journalForDrift(hostIds, since, max) : mockJournal.forDrift(hostIds, since, max),
  journalExport: (query: ChangeQuery): Promise<string | null> =>
    T() ? commands.journalExport(query) : Promise.resolve("/Users/you/Downloads/nightgrid-journal.md"),
  uptimeDays: (days: number, utcOffsetMin: number): Promise<UptimeSeries[]> => (T() ? commands.uptimeDays(days, utcOffsetMin) : mockUptime.days(days, utcOffsetMin)),
  uptimeOutages: (node: DepNode | null, from: number, to: number): Promise<UptimeOutage[]> =>
    T() ? commands.uptimeOutages(node, from, to) : mockUptime.outages(node, from, to),
  uptimeReport: (year: number, month: number, target: number): Promise<UptimeReport> =>
    T() ? commands.uptimeReport(year, month, target) : mockUptime.report(year, month, target),
  uptimeReportExport: (year: number, month: number, target: number): Promise<string | null> =>
    T() ? commands.uptimeReportExport(year, month, target) : Promise.resolve(`/Users/you/Downloads/nightgrid-uptime-${year}-${String(month).padStart(2, "0")}.html`),
  logSearch: (query: LogSearchQuery): Promise<LogSearchResult> => (T() ? commands.logSearch(query) : mockLogs.search(query)),
  logSearchesList: (): Promise<SavedLogSearch[]> => (T() ? commands.logSearchesList() : mockLogs.list()),
  logSearchSave: (input: SavedLogSearchInput): Promise<SavedLogSearch> => (T() ? commands.logSearchSave(input) : mockLogs.save(input)),
  logSearchDelete: (id: number): Promise<null> => (T() ? commands.logSearchDelete(id) : mockLogs.delete(id)),
  accessAudit: (hostId: number, elevate: boolean, sudo: SudoInput | null): Promise<AccessReport> =>
    T() ? commands.accessAudit(hostId, elevate, sudo) : mockAccess.audit(hostId, elevate),
  cronRead: (hostId: number, user: string | null, sudo: SudoInput | null): Promise<Crontab> => (T() ? commands.cronRead(hostId, user, sudo) : mockJobs.read(user)),
  cronApply: (hostId: number, user: string | null, hash: string, edit: CronEdit, sudo: SudoInput | null): Promise<Crontab> =>
    T() ? commands.cronApply(hostId, user, hash, edit, sudo) : mockJobs.apply(user, hash, edit),
  cronDescribe: (schedule: string): Promise<string | null> => (T() ? commands.cronDescribe(schedule) : mockJobs.describe(schedule)),
  timerJobs: (hostId: number): Promise<InstalledJob[]> => (T() ? commands.timerJobs(hostId) : mockJobs.timers()),
  timerJobSave: (hostId: number, job: TimerJob, replaces: string | null, sudo: SudoInput | null): Promise<InstalledJob> =>
    T() ? commands.timerJobSave(hostId, job, replaces, sudo) : mockJobs.saveTimer(job, replaces),
  timerJobRemove: (hostId: number, unit: string, sudo: SudoInput | null): Promise<null> => (T() ? commands.timerJobRemove(hostId, unit, sudo) : mockJobs.removeTimer(unit)),
  firewallRead: (hostId: number, sudo: SudoInput | null): Promise<FirewallView> => (T() ? commands.firewallRead(hostId, sudo) : mockFirewall.read(hostId)),
  firewallCheck: (hostId: number, change: FwChange): Promise<ChangeCheck> => (T() ? commands.firewallCheck(hostId, change) : mockFirewall.check(hostId, change)),
  firewallApply: (hostId: number, change: FwChange, secs: number, typed: string | null, sudo: SudoInput | null): Promise<PendingChange> =>
    T() ? commands.firewallApply(hostId, change, secs, typed, sudo) : mockFirewall.apply(hostId, change, secs, typed),
  firewallTest: (hostId: number): Promise<number> => (T() ? commands.firewallTest(hostId) : mockFirewall.test(hostId)),
  firewallKeep: (hostId: number, token: string): Promise<null> => (T() ? commands.firewallKeep(hostId, token) : mockFirewall.finish(hostId, token, true)),
  firewallUndo: (hostId: number, token: string): Promise<null> => (T() ? commands.firewallUndo(hostId, token) : mockFirewall.finish(hostId, token, false)),
  proxyEditTargets: (hostId: number): Promise<ProxyEditTargets> => (T() ? commands.proxyEditTargets(hostId) : mockProxyEdit.targets(hostId)),
  caddyEditPreview: (text: string, edit: CaddyEdit): Promise<CaddyPreview> => (T() ? commands.caddyEditPreview(text, edit) : mockProxyEdit.preview(text, edit)),
  caddySave: (hostId: number, proxyName: string, text: string, baseHash: string, summary: string, sudo: SudoInput | null): Promise<SaveOutcome> =>
    T() ? commands.caddySave(hostId, proxyName, text, baseHash, summary, sudo) : mockProxyEdit.saveCaddy(hostId, proxyName, text, baseHash),
  traefikSave: (hostId: number, proxyName: string, routes: TraefikRoute[], baseHash: string): Promise<TraefikTarget> =>
    T() ? commands.traefikSave(hostId, proxyName, routes, baseHash) : mockProxyEdit.saveTraefik(hostId, proxyName, routes, baseHash),
  envVarsList: (hostId: number, stack: string): Promise<EnvVar[]> => (T() ? commands.envVarsList(hostId, stack) : mockEnvVault.list(hostId, stack)),
  envStacks: (): Promise<[number, string, number][]> => (T() ? commands.envStacks() : mockEnvVault.stacks()),
  envVarSet: (hostId: number, stack: string, oldName: string | null, input: EnvVarInput): Promise<EnvVar> =>
    T() ? commands.envVarSet(hostId, stack, oldName, input) : mockEnvVault.set(hostId, stack, oldName, input),
  envVarDelete: (id: number): Promise<null> => (T() ? commands.envVarDelete(id) : mockEnvVault.remove(id)),
  envVarReveal: (id: number): Promise<string> => (T() ? commands.envVarReveal(id) : mockEnvVault.reveal(id)),
  envVarsOrder: (hostId: number, stack: string, ids: number[]): Promise<null> =>
    T() ? commands.envVarsOrder(hostId, stack, ids) : mockEnvVault.order(hostId, stack, ids),
  envImportHost: (hostId: number, stack: Stack): Promise<number> => (T() ? commands.envImportHost(hostId, stack) : mockEnvVault.importHost(hostId, stack)),
  envDeployPreview: (hostId: number, stack: Stack): Promise<EnvDiff> => (T() ? commands.envDeployPreview(hostId, stack) : mockEnvVault.preview(hostId, stack)),
  envDeploy: (hostId: number, stack: Stack): Promise<EnvDeployed> => (T() ? commands.envDeploy(hostId, stack) : mockEnvVault.deploy(hostId, stack)),
  gitStacksList: (hostId: number): Promise<GitStack[]> => (T() ? commands.gitStacksList(hostId) : mockGitStacks.list(hostId)),
  gitStackAdd: (hostId: number, input: GitStackInput): Promise<GitStackAdded> => (T() ? commands.gitStackAdd(hostId, input) : mockGitStacks.add(hostId, input)),
  gitStackCheck: (id: number): Promise<GitStatus> => (T() ? commands.gitStackCheck(id) : mockGitStacks.check(id)),
  gitStackDiff: (id: number, target: string): Promise<string> => (T() ? commands.gitStackDiff(id, target) : mockGitStacks.diff(id, target)),
  gitStackDeploy: (id: number, target: string): Promise<GitDeployed> => (T() ? commands.gitStackDeploy(id, target) : mockGitStacks.deploy(id, target)),
  gitStackRemove: (id: number): Promise<null> => (T() ? commands.gitStackRemove(id) : mockGitStacks.remove(id)),
  netTestRun: (fromHost: number, toHost: number, target: string | null, secs: number): Promise<NetTestResult> =>
    T() ? commands.netTestRun(fromHost, toHost, target, secs) : mockNetperf.run(fromHost, toHost, target, secs),
  pveOverview: (hostId: number, sudo: SudoInput | null): Promise<PveOverview> => (T() ? commands.pveOverview(hostId, sudo) : mockProxmox.overview(hostId)),
  pveGuestAction: (hostId: number, node: string, kind: string, vmid: number, action: PveAction, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveGuestAction(hostId, node, kind, vmid, action, sudo) : mockProxmox.action(hostId, node, kind, vmid, action),
  pveTask: (hostId: number, node: string, upid: string, sudo: SudoInput | null): Promise<PveTaskDetail> =>
    T() ? commands.pveTask(hostId, node, upid, sudo) : mockProxmox.task(hostId, node, upid),
  pveSnapshots: (hostId: number, node: string, kind: string, vmid: number, sudo: SudoInput | null): Promise<PveSnapshot[]> =>
    T() ? commands.pveSnapshots(hostId, node, kind, vmid, sudo) : mockProxmox.snapshots(node, vmid),
  pveSnapshotCreate: (hostId: number, node: string, kind: string, vmid: number, name: string, description: string, vmstate: boolean, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveSnapshotCreate(hostId, node, kind, vmid, name, description, vmstate, sudo) : mockProxmox.snapshotCreate(node, vmid, name, description, vmstate),
  pveSnapshotRollback: (hostId: number, node: string, kind: string, vmid: number, name: string, start: boolean, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveSnapshotRollback(hostId, node, kind, vmid, name, start, sudo) : mockProxmox.snapshotRollback(node, vmid, name),
  pveSnapshotDelete: (hostId: number, node: string, kind: string, vmid: number, name: string, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveSnapshotDelete(hostId, node, kind, vmid, name, sudo) : mockProxmox.snapshotDelete(node, vmid, name),
  pveBackups: (hostId: number, storages: PveStorageRef[], sudo: SudoInput | null): Promise<PveBackups> => (T() ? commands.pveBackups(hostId, storages, sudo) : mockProxmox.backups()),
  pveBackupNow: (hostId: number, node: string, vmid: number, storage: string, mode: string, notes: string, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveBackupNow(hostId, node, vmid, storage, mode, notes, sudo) : mockProxmox.backupNow(node, vmid, storage),
  pveRestore: (hostId: number, node: string, kind: string, vmid: number, volid: string, storage: string, force: boolean, start: boolean, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveRestore(hostId, node, kind, vmid, volid, storage, force, start, sudo) : mockProxmox.restore(node, kind, vmid, volid),
  pveBackupProtect: (hostId: number, node: string, volid: string, protect: boolean, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.pveBackupProtect(hostId, node, volid, protect, sudo) : mockProxmox.protect(volid, protect),
  pveBackupDelete: (hostId: number, volid: string, sudo: SudoInput | null): Promise<null> => (T() ? commands.pveBackupDelete(hostId, volid, sudo) : mockProxmox.deleteBackup(volid)),
  pveCreateInfo: (hostId: number, node: string, isoStorages: string[], tmplStorages: string[], sudo: SudoInput | null): Promise<PveCreateInfo> =>
    T() ? commands.pveCreateInfo(hostId, node, isoStorages, tmplStorages, sudo) : mockProxmox.createInfo(),
  pveVmCreate: (hostId: number, spec: PveVmSpec, sudo: SudoInput | null): Promise<string> => (T() ? commands.pveVmCreate(hostId, spec, sudo) : mockProxmox.vmCreate(spec)),
  pveCtCreate: (hostId: number, spec: PveCtSpec, sudo: SudoInput | null): Promise<string> => (T() ? commands.pveCtCreate(hostId, spec, sudo) : mockProxmox.ctCreate(spec)),
  pveClone: (hostId: number, spec: PveCloneSpec, sudo: SudoInput | null): Promise<string> => (T() ? commands.pveClone(hostId, spec, sudo) : mockProxmox.clone(spec)),
  questCheck: (hostId: number, checks: string[]): Promise<QuestResult[]> => (T() ? commands.questCheck(hostId, checks) : mockQuest.check(checks)),
  questsList: (): Promise<Quest[]> => (T() ? commands.questsList() : mockQuestLog.list()),
  questStart: (articleId: string, hostId: number | null, title: string, ticks: string[]): Promise<Quest> =>
    T() ? commands.questStart(articleId, hostId, title, ticks) : mockQuestLog.start(articleId, hostId, title, ticks),
  questTicks: (id: number, ticks: string[], complete: boolean): Promise<Quest> => (T() ? commands.questTicks(id, ticks, complete) : mockQuestLog.ticks(id, ticks, complete)),
  questAbandon: (id: number): Promise<boolean> => (T() ? commands.questAbandon(id) : mockQuestLog.abandon(id)),
  nodeList: (): Promise<NodeView[]> => (T() ? commands.nodeList() : mockNode.list()),
  nodeCheck: (hostId: number): Promise<NodeView> => (T() ? commands.nodeCheck(hostId) : mockNode.check(hostId)),
  nodeSyncNow: (hostId: number): Promise<SyncReport> => (T() ? commands.nodeSyncNow(hostId) : mockNode.syncNow(hostId)),
  nodeSetUse: (hostId: number, on: boolean): Promise<NodeView> => (T() ? commands.nodeSetUse(hostId, on) : mockNode.setUse(hostId, on)),
  nodeRelease: (): Promise<Release> => (T() ? commands.nodeRelease() : mockNode.release()),
  nodeInstall: (hostId: number, fromFile: boolean, sudo: SudoInput | null): Promise<NodeInstallResult | null> =>
    T() ? commands.nodeInstall(hostId, fromFile, sudo) : mockNode.install(hostId),
  nodeUninstall: (hostId: number, purge: boolean, sudo: SudoInput | null): Promise<NodeInstallResult> =>
    T() ? commands.nodeUninstall(hostId, purge, sudo) : mockNode.uninstall(hostId),
  nodeSetup: (hostId: number): Promise<NodeSetup> => (T() ? commands.nodeSetup(hostId) : mockHub.setup(hostId)),
  nodeConfigRead: (hostId: number, part: NodePart): Promise<NodeConfigFile> => (T() ? commands.nodeConfigRead(hostId, part) : mockHub.configRead(hostId, part)),
  nodeConfigSave: (hostId: number, part: NodePart, text: string, sudo: SudoInput | null): Promise<ConfigSaveReport> =>
    T() ? commands.nodeConfigSave(hostId, part, text, sudo) : mockHub.configSave(hostId, part, text),
  nodeLogs: (hostId: number, part: NodePart, lines: number, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.nodeLogs(hostId, part, lines, sudo) : mockHub.logs(part),
  hubInstall: (hostId: number, fromFile: boolean, sudo: SudoInput | null): Promise<HubStepResult | null> =>
    T() ? commands.hubInstall(hostId, fromFile, sudo) : mockHub.install(hostId),
  hubUninstall: (hostId: number, purge: boolean, sudo: SudoInput | null): Promise<HubStepResult> =>
    T() ? commands.hubUninstall(hostId, purge, sudo) : mockHub.uninstall(hostId),
  hubJoin: (memberId: number, hubHost: number, sudo: SudoInput | null): Promise<HubJoinResult> =>
    T() ? commands.hubJoin(memberId, hubHost, sudo) : mockHub.join(memberId, hubHost),
  hubLeave: (memberId: number, hubHost: number, sudo: SudoInput | null): Promise<HubJoinResult> =>
    T() ? commands.hubLeave(memberId, hubHost, sudo) : mockHub.leave(memberId),
  hubAllowNetworks: (hubHost: number, networks: string[], listen: string[], sudo: SudoInput | null): Promise<ConfigSaveReport> =>
    T() ? commands.hubAllowNetworks(hubHost, networks, listen, sudo) : mockHub.allowNetworks(networks, listen),
  nodeLinkEnable: (hostId: number, sudo: SudoInput | null): Promise<ConfigSaveReport> =>
    T() ? commands.nodeLinkEnable(hostId, sudo) : mockHub.linkEnable(hostId),
  hubRepairLink: (memberId: number, hubHost: number, sudo: SudoInput | null): Promise<ConfigSaveReport> =>
    T() ? commands.hubRepairLink(memberId, hubHost, sudo) : mockHub.repairLink(memberId),
  powerOverview: (days: number, utcOffsetMins: number): Promise<PowerOverview> => (T() ? commands.powerOverview(days, utcOffsetMins) : mockPower.overview(days)),
  powerProfileSet: (hostId: number, idleW: number, maxW: number): Promise<null> => (T() ? commands.powerProfileSet(hostId, idleW, maxW) : mockPower.set(hostId, idleW, maxW)),
  powerProfileClear: (hostId: number): Promise<null> => (T() ? commands.powerProfileClear(hostId) : mockPower.clear(hostId)),
  pveSandboxCreate: (hostId: number, spec: PveSandboxSpec, sudo: SudoInput | null): Promise<string> => (T() ? commands.pveSandboxCreate(hostId, spec, sudo) : mockProxmox.sandboxCreate(spec)),
  pveSandboxIsolate: (hostId: number, spec: PveSandboxSpec, sudo: SudoInput | null): Promise<PveSandboxDone> => (T() ? commands.pveSandboxIsolate(hostId, spec, sudo) : mockProxmox.sandboxIsolate(spec)),
  pveSandboxExtend: (hostId: number, node: string, kind: string, vmid: number, hours: number, sudo: SudoInput | null): Promise<number | null> =>
    T() ? commands.pveSandboxExtend(hostId, node, kind, vmid, hours, sudo) : mockProxmox.sandboxExtend(vmid, hours),
  pveSandboxKeep: (hostId: number, node: string, kind: string, vmid: number, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.pveSandboxKeep(hostId, node, kind, vmid, sudo) : mockProxmox.sandboxKeep(vmid),
  pveSandboxDestroy: (hostId: number, node: string, kind: string, vmid: number, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveSandboxDestroy(hostId, node, kind, vmid, sudo) : mockProxmox.sandboxDestroy(node, vmid),
  pveTemplate: (hostId: number, node: string, kind: string, vmid: number, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveTemplate(hostId, node, kind, vmid, sudo) : mockProxmox.template(node, vmid),
  pveDestroy: (hostId: number, node: string, kind: string, vmid: number, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveDestroy(hostId, node, kind, vmid, sudo) : mockProxmox.destroy(node, vmid),
  pveIsoDownload: (hostId: number, node: string, storage: string, url: string, filename: string, sha256: string | null, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveIsoDownload(hostId, node, storage, url, filename, sha256, sudo) : mockProxmox.isoDownload(node, storage, filename),
  pveConfig: (hostId: number, node: string, kind: string, vmid: number, sudo: SudoInput | null): Promise<PveConfig> =>
    T() ? commands.pveConfig(hostId, node, kind, vmid, sudo) : mockProxmox.config(vmid, kind),
  pveConfigSet: (hostId: number, node: string, kind: string, vmid: number, digest: string, set: PveSetting[], remove: string[], sudo: SudoInput | null): Promise<null> =>
    T() ? commands.pveConfigSet(hostId, node, kind, vmid, digest, set, remove, sudo) : mockProxmox.configSet(vmid, digest, set, remove),
  pveDiskResize: (hostId: number, node: string, kind: string, vmid: number, disk: string, growGb: number, sudo: SudoInput | null): Promise<string | null> =>
    T() ? commands.pveDiskResize(hostId, node, kind, vmid, disk, growGb, sudo) : mockProxmox.diskResize(vmid, disk, growGb),
  pveHostDevices: (hostId: number, node: string, sudo: SudoInput | null): Promise<PveHostDevices> => (T() ? commands.pveHostDevices(hostId, node, sudo) : mockProxmox.hostDevices()),
  pveNetwork: (hostId: number, node: string, sudo: SudoInput | null): Promise<PveNetwork> => (T() ? commands.pveNetwork(hostId, node, sudo) : mockProxmox.network()),
  pveIfaceSave: (hostId: number, node: string, spec: PveIfaceSpec, create: boolean, sudo: SudoInput | null): Promise<null> =>
    T() ? commands.pveIfaceSave(hostId, node, spec, create, sudo) : mockProxmox.ifaceSave(spec, create),
  pveIfaceDelete: (hostId: number, node: string, iface: string, sudo: SudoInput | null): Promise<null> => (T() ? commands.pveIfaceDelete(hostId, node, iface, sudo) : mockProxmox.ifaceDelete(iface)),
  pveNetworkRevert: (hostId: number, node: string, sudo: SudoInput | null): Promise<null> => (T() ? commands.pveNetworkRevert(hostId, node, sudo) : mockProxmox.networkRevert()),
  pveNetworkApply: (hostId: number, node: string, sudo: SudoInput | null): Promise<PveNetApply> => (T() ? commands.pveNetworkApply(hostId, node, sudo) : mockProxmox.networkApply(node)),
  pveNetworkSettle: (hostId: number, token: string, keep: boolean, sudo: SudoInput | null): Promise<null> => (T() ? commands.pveNetworkSettle(hostId, token, keep, sudo) : mockProxmox.networkSettle(keep)),
  pveFirewall: (hostId: number, scope: PveFwScope, sudo: SudoInput | null): Promise<PveFirewall> => (T() ? commands.pveFirewall(hostId, scope, sudo) : mockProxmox.firewall(scope)),
  pveFwRuleSave: (hostId: number, scope: PveFwScope, local: string, token: string | null, pos: number | null, digest: string, rule: PveFwRuleSpec, sudo: SudoInput | null): Promise<PveFwRun> =>
    T() ? commands.pveFwRuleSave(hostId, scope, local, token, pos, digest, rule, sudo) : mockProxmox.fwRuleSave(scope, token, pos, rule),
  pveFwRuleMove: (hostId: number, scope: PveFwScope, local: string, token: string | null, pos: number, to: number, digest: string, sudo: SudoInput | null): Promise<PveFwRun> =>
    T() ? commands.pveFwRuleMove(hostId, scope, local, token, pos, to, digest, sudo) : mockProxmox.fwRuleMove(scope, token, pos, to),
  pveFwRuleDelete: (hostId: number, scope: PveFwScope, local: string, token: string | null, pos: number, digest: string, sudo: SudoInput | null): Promise<PveFwRun> =>
    T() ? commands.pveFwRuleDelete(hostId, scope, local, token, pos, digest, sudo) : mockProxmox.fwRuleDelete(scope, token, pos),
  pveFwOptions: (hostId: number, scope: PveFwScope, local: string, token: string | null, enable: boolean, policyIn: string, policyOut: string, digest: string, sudo: SudoInput | null): Promise<PveFwRun> =>
    T() ? commands.pveFwOptions(hostId, scope, local, token, enable, policyIn, policyOut, digest, sudo) : mockProxmox.fwOptions(scope, token, enable, policyIn, policyOut),
  pveFwObject: (hostId: number, local: string, token: string | null, op: string, name: string, cidr: string, comment: string, sudo: SudoInput | null): Promise<PveFwRun> =>
    T() ? commands.pveFwObject(hostId, local, token, op, name, cidr, comment, sudo) : mockProxmox.fwObject(token, op, name, cidr, comment),
  pveFwSettle: (hostId: number, token: string, keep: boolean, sudo: SudoInput | null): Promise<null> => (T() ? commands.pveFwSettle(hostId, token, keep, sudo) : mockProxmox.fwSettle(keep)),
  pveAppliances: (hostId: number, node: string, sudo: SudoInput | null): Promise<PveAppliance[]> => (T() ? commands.pveAppliances(hostId, node, sudo) : mockProxmox.appliances()),
  pveApplianceDownload: (hostId: number, node: string, storage: string, template: string, sudo: SudoInput | null): Promise<string> =>
    T() ? commands.pveApplianceDownload(hostId, node, storage, template, sudo) : mockProxmox.applianceDownload(node, storage, template),
  cmdWidgetsList: (): Promise<CmdWidget[]> => (T() ? commands.cmdWidgetsList() : mockCmdWidgets.list()),
  cmdWidgetSave: (widget: CmdWidget): Promise<CmdWidget> => (T() ? commands.cmdWidgetSave(widget) : mockCmdWidgets.save(widget)),
  cmdWidgetDelete: (id: number): Promise<boolean> => (T() ? commands.cmdWidgetDelete(id) : mockCmdWidgets.remove(id)),
  cmdWidgetRun: (id: number): Promise<CmdReading> => (T() ? commands.cmdWidgetRun(id) : mockCmdWidgets.run(id)),
  cmdWidgetTry: (widget: CmdWidget): Promise<CmdReading> => (T() ? commands.cmdWidgetTry(widget) : mockCmdWidgets.tryRun(widget)),
  netTestsList: (): Promise<NetTest[]> => (T() ? commands.netTestsList() : mockNetperf.list()),
  netTestsClear: (fromHost: number, toHost: number): Promise<number> => (T() ? commands.netTestsClear(fromHost, toHost) : mockNetperf.clear(fromHost, toHost)),
  timerCalendarPreview: (hostId: number, calendar: string): Promise<CalendarPreview> => (T() ? commands.timerCalendarPreview(hostId, calendar) : mockJobs.preview(calendar)),
  alertsLogSamples: (key: string): Promise<string[]> => (T() ? commands.alertsLogSamples(key) : mockOps.logSamples(key)),
  logClassify: (text: string): Promise<LogEventView[]> => (T() ? commands.logClassify(text) : Promise.resolve(mockAlerts.classify(text))),
  alertsLogMute: (hostId: number, source: string, fingerprint: string): Promise<AppSettings> =>
    T()
      ? commands.alertsLogMute(hostId, source, fingerprint)
      : mock.settingsGet().then((cur) => {
          mockAlerts.dropNote(source, fingerprint);
          const mutes = cur.alerts.logMutes ?? [];
          const label = `pattern ${fingerprint}`;
          return mock.settingsSet({ ...cur, alerts: { ...cur.alerts, logMutes: [...mutes, { source, regex: "", fingerprint, label }] } });
        }),
  updateCheck: (): Promise<UpdateStatus> => (T() ? commands.updateCheck() : Promise.resolve({ kind: "notConfigured" })),
  broadcastStatus: (): Promise<BroadcastStatus> => (T() ? commands.broadcastStatus() : Promise.resolve(mockBroadcast.status())),
  broadcastCheck: (): Promise<BroadcastStatus> => (T() ? commands.broadcastCheck() : Promise.resolve(mockBroadcast.status())),
  kbPackStatus: (): Promise<KbPackStatus> => (T() ? commands.kbPackStatus() : Promise.resolve(NO_KB_PACK)),
  kbPackCheck: (bundled: number): Promise<KbPackStatus> => (T() ? commands.kbPackCheck(bundled) : Promise.resolve(NO_KB_PACK)),
  kbPackLoad: (files: boolean): Promise<KbPackContent | null> => (T() ? commands.kbPackLoad(files) : Promise.resolve(null)),
  kbPackRemove: (): Promise<KbPackStatus> => (T() ? commands.kbPackRemove() : Promise.resolve(NO_KB_PACK)),
  componentPacks: (): Promise<ComponentPackInfo[]> => (T() ? commands.componentPacks() : Promise.resolve(mockComponentPacks())),
  componentPacksCheck: (ids: string[]): Promise<ComponentPackCheck> =>
    T() ? commands.componentPacksCheck(ids) : Promise.resolve({ packs: mockComponentPacks(), updated: [], checkedAt: null, error: null, notConfigured: true }),
  componentPackRemove: (id: string): Promise<ComponentPackInfo[]> => (T() ? commands.componentPackRemove(id) : Promise.resolve(mockComponentPacks().filter((p) => p.id !== id))),
  uiPatchStatus: (): Promise<UiPatchStatus> => (T() ? commands.uiPatchStatus() : Promise.resolve(NO_UI_PATCH)),
  uiPatchReady: (): Promise<void> => (T() ? commands.uiPatchReady() : Promise.resolve()),
  uiPatchCheck: (): Promise<UiPatchStatus> => (T() ? commands.uiPatchCheck() : Promise.resolve(NO_UI_PATCH)),
  uiPatchRemove: (): Promise<UiPatchStatus> => (T() ? commands.uiPatchRemove() : Promise.resolve(NO_UI_PATCH)),
  uiPatchRestart: (): Promise<void> => (T() ? commands.uiPatchRestart() : Promise.resolve()),
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
  noticesList: (limit: number): Promise<Notice[]> => (T() ? commands.noticesList(limit) : mockNotify.list(limit)),
  noticesRead: (ids: number[] | null): Promise<null> => (T() ? commands.noticesRead(ids) : mockNotify.read(ids)),
  noticesClear: (): Promise<null> => (T() ? commands.noticesClear() : mockNotify.clear()),
  ntfyGet: (): Promise<NtfyInfo> => (T() ? commands.ntfyGet() : mockNotify.ntfyGet()),
  ntfySet: (url: string | null, token: string | null): Promise<NtfyInfo> => (T() ? commands.ntfySet(url, token) : mockNotify.ntfySet(url, token)),
  notifyTest: (): Promise<NotifyTest> => (T() ? commands.notifyTest() : mockNotify.test()),
  postureList: (): Promise<PostureHost[]> => (T() ? commands.postureList() : mockPosture.list()),
  postureHistory: (hostId: number): Promise<PostureRun[]> => (T() ? commands.postureHistory(hostId) : mockPosture.history(hostId)),
  postureCheck: (hostIds: number[]): Promise<PostureRun[]> => (T() ? commands.postureCheck(hostIds) : mockPosture.check(hostIds)),
  secretsList: (): Promise<SecretItem[]> => (T() ? commands.secretsList() : mockSecrets.list()),
  secretMarkRotated: (key: string): Promise<null> => (T() ? commands.secretMarkRotated(key) : mockSecrets.markRotated(key)),
  secretPolicy: (key: string, days: number | null): Promise<null> => (T() ? commands.secretPolicy(key, days) : mockSecrets.policy(key, days)),
  secretManualSave: (secret: ManualSecret): Promise<number> => (T() ? commands.secretManualSave(secret) : mockSecrets.save(secret)),
  secretManualDelete: (id: number): Promise<null> => (T() ? commands.secretManualDelete(id) : mockSecrets.remove(id)),
  secretKeyHosts: (fingerprint: string): Promise<KeyHost[]> => (T() ? commands.secretKeyHosts(fingerprint) : mockSecrets.keyHosts(fingerprint)),
  catalogBuild: (hostIds: number[]): Promise<CatalogHost[]> => (T() ? commands.catalogBuild(hostIds) : mockCatalog.build(hostIds)),
  catalogMarkdown: (hosts: CatalogHost[]): Promise<string> => (T() ? commands.catalogMarkdown(hosts) : mockCatalog.markdown(hosts)),
  reportSave: (name: string, content: string): Promise<string | null> => (T() ? commands.reportSave(name, content) : Promise.resolve(null)),
  drillsList: (): Promise<DrillBoard> => (T() ? commands.drillsList() : mockDrills.list()),
  drillSave: (drill: Drill): Promise<number> => (T() ? commands.drillSave(drill) : mockDrills.save(drill)),
  drillDelete: (id: number): Promise<null> => (T() ? commands.drillDelete(id) : mockDrills.remove(id)),
  drillRuns: (id: number): Promise<DrillRun[]> => (T() ? commands.drillRuns(id) : mockDrills.runs(id)),
  drillRun: (id: number): Promise<DrillRun> => (T() ? commands.drillRun(id) : mockDrills.run(id)),
  drillReport: (id: number): Promise<string> => (T() ? commands.drillReport(id) : mockDrills.report(id)),
  capacityList: (days: number): Promise<HostCapacity[]> => (T() ? commands.capacityList(days) : mockCapacity.list(days)),
  replayIncidents: (days: number): Promise<Incident[]> => (T() ? commands.replayIncidents(days) : mockReplay.incidents()),
  replayLoad: (incident: Incident): Promise<Replay> => (T() ? commands.replayLoad(incident) : mockReplay.load(incident)),
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
