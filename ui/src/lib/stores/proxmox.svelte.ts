/**
 * Proxmox VE (M10r): one host's view of its cluster through `pvesh`, read as
 * root (the sudo prompt asks when needed). Power actions start Proxmox tasks;
 * the task's log is followed until it ends, then the view reloads.
 */
import { sound } from "./sound.svelte";
import { api, errorText, type PveFirewall, type PveFwRun, type PveFwRuleSpec, type PveFwScope, type PveIfaceSpec, type PveNetwork, type PveAppliance, type PveCloneSpec, type PveSandboxSpec, type PveCreateInfo, type PveCtSpec, type PveOverview, type PveTaskDetail, type PveVmSpec, type PresetGuest } from "../api";
import { ACTION_WORD, backupTargets, type PveConfig, type PveHostDevices, type PveSetting, type PveAction, type PveBackupFile, type PveBackups, type PveGuest, type PveSnapshot, type PveTask } from "../proxmox";
import { hosts } from "./hosts.svelte";
import { system } from "./system.svelte";

export type PveTab = "summary" | "guests" | "sandbox" | "backups" | "storage" | "network" | "firewall" | "tasks";

/** A destructive step waiting for a confirmation that names its target (rule 5). */
export interface PveConfirm {
  title: string;
  body: string;
  button: string;
  /** When set, this text must be typed to enable the button. */
  typed?: string;
  run: () => Promise<void>;
}

/** Which "make something" dialog is open. */
/** `preset` fills the create form from a starter (PresetGuest). */
export type PveMake = { kind: "vm"; preset?: PresetGuest } | { kind: "ct"; preset?: PresetGuest } | { kind: "clone"; guest: PveGuest } | { kind: "sandbox"; guest: PveGuest | null } | { kind: "iso" } | { kind: "appliance" };

export interface GuestDetail {
  guest: PveGuest;
  tab: "snapshots" | "backups" | "hardware" | "firewall";
  snaps: PveSnapshot[] | null;
  error: string | null;
  config: PveConfig | null;
  configError: string | null;
  devices: PveHostDevices | null;
}
const HOST_KEY = "nightgrid.proxmox.host";

function remembered(): number | null {
  try {
    const v = Number(localStorage.getItem(HOST_KEY));
    return Number.isFinite(v) && v > 0 ? v : null;
  } catch {
    return null;
  }
}

export interface OpenTask {
  node: string;
  upid: string;
  title: string;
  detail: PveTaskDetail | null;
  error: string | null;
  onDone?: (d: PveTaskDetail) => void;
  /** Closed by the user while it still runs: followed quietly to the end. */
  hidden?: boolean;
}

/** "datacenter", "node pve1", "VM 100", "group webservers". */
export function scopeName(s: PveFwScope): string {
  return s.level === "cluster" ? "datacenter" : s.level === "node" ? `node ${s.node}` : s.level === "guest" ? `${s.kind === "qemu" ? "VM" : "CT"} ${s.vmid}` : `group ${s.group}`;
}

class ProxmoxStore {
  hostId = $state<number | null>(remembered());
  overview = $state<PveOverview | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  tab = $state<PveTab>("summary");
  query = $state("");
  node = $state<string | null>(null);
  pending = $state<{ guest: PveGuest; action: PveAction } | null>(null);
  busy = $state<number[]>([]);
  notice = $state<{ text: string; tone: "ok" | "err" } | null>(null);
  task = $state<OpenTask | null>(null);
  detail = $state<GuestDetail | null>(null);
  backups = $state<PveBackups | null>(null);
  backupsLoading = $state(false);
  backupsError = $state<string | null>(null);
  confirm = $state<PveConfirm | null>(null);
  make = $state<PveMake | null>(null);
  createInfo = $state<PveCreateInfo | null>(null);
  createInfoError = $state<string | null>(null);
  appliances = $state<PveAppliance[] | null>(null);
  network = $state<PveNetwork | null>(null);
  networkError = $state<string | null>(null);
  /** An applied network change waiting for "keep" before the node rolls it back. */
  /** Host-level firewall changes waiting for "keep" (one rollback covers the whole session). */
  fwSession = $state<{ token: string; until: number } | null>(null);
  netApply = $state<{ token: string; node: string; until: number; upid: string | null } | null>(null);
  private seq = 0;
  private follow: ReturnType<typeof setTimeout> | null = null;

  get host() {
    return hosts.hosts.find((h) => h.id === this.hostId) ?? null;
  }

  pick(id: number | null) {
    if (id === this.hostId) return;
    this.hostId = id;
    this.overview = null;
    this.error = null;
    this.notice = null;
    this.node = null;
    this.backups = null;
    this.detail = null;
    this.createInfo = null;
    this.appliances = null;
    this.network = null;
    this.networkError = null;
    this.stopFollowing();
    try {
      if (id) localStorage.setItem(HOST_KEY, String(id));
    } catch {
      /* per-viewer convenience only */
    }
    if (id) void this.load();
  }

  private sudo<T>(reason: string, f: (s: Parameters<typeof api.pveOverview>[1]) => Promise<T>): Promise<T> {
    return system.withSudo(reason, f, this.host?.name);
  }

  async load() {
    const id = this.hostId;
    if (id === null) return;
    const seq = ++this.seq;
    this.loading = true;
    try {
      const o = await this.sudo("read Proxmox", (s) => api.pveOverview(id, s));
      if (seq !== this.seq) return;
      this.overview = o;
      this.error = null;
    } catch (e) {
      if (seq === this.seq) this.error = errorText(e);
    } finally {
      if (seq === this.seq) this.loading = false;
    }
  }

  /** Destructive actions wait for confirmation (rule 5); start and resume run at once. */
  request(guest: PveGuest, action: PveAction) {
    if (action === "start" || action === "resume") void this.act(guest, action);
    else this.pending = { guest, action };
  }

  async act(guest: PveGuest, action: PveAction) {
    const id = this.hostId;
    if (id === null) return;
    this.pending = null;
    this.busy = [...this.busy, guest.vmid];
    const what = `${ACTION_WORD[action]} ${guest.name || guest.vmid}`;
    try {
      const upid = await this.sudo(what.toLowerCase(), (s) => api.pveGuestAction(id, guest.node, guest.kind, guest.vmid, action, s));
      this.notice = { text: `${what}: started.`, tone: "ok" };
      void this.openTask(guest.node, upid, what);
    } catch (e) {
      this.notice = { text: `${what}: ${errorText(e)}`, tone: "err" };
    } finally {
      this.busy = this.busy.filter((v) => v !== guest.vmid);
    }
  }

  /** Shows a task's log and follows it while it runs. */
  async openTask(node: string, upid: string, title: string, onDone?: (d: PveTaskDetail) => void) {
    this.stopFollowing();
    this.task = { node, upid, title, detail: null, error: null, onDone };
    await this.poll();
  }

  openListed(t: PveTask, title: string) {
    void this.openTask(t.node, t.upid, title);
  }

  private async poll() {
    const t = this.task;
    const id = this.hostId;
    if (!t || id === null) return;
    try {
      const d = await this.sudo("read a Proxmox task", (s) => api.pveTask(id, t.node, t.upid, s));
      if (this.task?.upid !== t.upid) return;
      this.task = { ...this.task, detail: d, error: null };
      if (d.running) this.follow = setTimeout(() => void this.poll(), 1500);
      else {
        void this.load();
        t.onDone?.(d);
        if (this.task?.hidden) this.task = null;
      }
    } catch (e) {
      if (this.task?.upid === t.upid) this.task = { ...this.task, error: errorText(e) };
    }
  }

  // ---- part 2: snapshots and backups ----

  /** Runs a step that starts a Proxmox task, then follows it. */
  private async startTask(what: string, node: string, f: (s: Parameters<typeof api.pveOverview>[1]) => Promise<string>, onDone?: (d: PveTaskDetail) => void) {
    const id = this.hostId;
    if (id === null) return;
    try {
      const upid = await this.sudo(what.toLowerCase(), f);
      this.notice = { text: `${what}: started.`, tone: "ok" };
      void this.openTask(node, upid, what, onDone);
    } catch (e) {
      this.notice = { text: `${what}: ${errorText(e)}`, tone: "err" };
      throw e;
    }
  }

  openGuest(guest: PveGuest, tab: GuestDetail["tab"] = "snapshots") {
    this.detail = { guest, tab, snaps: null, error: null, config: null, configError: null, devices: null };
    void this.loadSnaps();
    if (tab === "hardware") void this.loadConfig();
    if (!this.backups) void this.loadBackups();
  }

  async loadSnaps() {
    const d = this.detail;
    const id = this.hostId;
    if (!d || id === null) return;
    try {
      const snaps = await this.sudo("read snapshots", (s) => api.pveSnapshots(id, d.guest.node, d.guest.kind, d.guest.vmid, s));
      if (this.detail?.guest.vmid === d.guest.vmid) this.detail = { ...this.detail, snaps, error: null };
    } catch (e) {
      if (this.detail?.guest.vmid === d.guest.vmid) this.detail = { ...this.detail, error: errorText(e) };
    }
  }

  async takeSnapshot(g: PveGuest, name: string, description: string, vmstate: boolean) {
    const id = this.hostId;
    if (id === null) return;
    await this.startTask(`Snapshot ${name} of ${g.name}`, g.node, (s) => api.pveSnapshotCreate(id, g.node, g.kind, g.vmid, name, description, vmstate, s), () => void this.loadSnaps());
  }

  askRollback(g: PveGuest, snap: PveSnapshot, start: boolean) {
    const id = this.hostId;
    if (id === null) return;
    this.confirm = {
      title: "ROLL BACK",
      body: `Roll ${g.name} (${g.vmid}) on ${g.node} back to snapshot “${snap.name}”? Everything that changed in the guest since that snapshot is lost${g.status === "running" ? ", and the guest is stopped first" : ""}.`,
      button: `Roll back to ${snap.name}`,
      run: () => this.startTask(`Roll back ${g.name} to ${snap.name}`, g.node, (s) => api.pveSnapshotRollback(id, g.node, g.kind, g.vmid, snap.name, start, s), () => void this.loadSnaps()),
    };
  }

  askDeleteSnapshot(g: PveGuest, snap: PveSnapshot) {
    const id = this.hostId;
    if (id === null) return;
    this.confirm = {
      title: "DELETE SNAPSHOT",
      body: `Delete snapshot “${snap.name}” of ${g.name} (${g.vmid})? The guest itself is untouched, but you can no longer go back to this point.`,
      button: `Delete ${snap.name}`,
      run: () => this.startTask(`Delete snapshot ${snap.name}`, g.node, (s) => api.pveSnapshotDelete(id, g.node, g.kind, g.vmid, snap.name, s), () => void this.loadSnaps()),
    };
  }

  async loadBackups() {
    const id = this.hostId;
    if (id === null) return;
    this.backupsLoading = true;
    try {
      this.backups = await this.sudo("read backups", (s) => api.pveBackups(id, backupTargets(this.overview), s));
      this.backupsError = null;
    } catch (e) {
      this.backupsError = errorText(e);
    } finally {
      this.backupsLoading = false;
    }
  }

  async backupNow(g: PveGuest, storage: string, mode: string, notes: string) {
    const id = this.hostId;
    if (id === null) return;
    await this.startTask(`Back up ${g.name}`, g.node, (s) => api.pveBackupNow(id, g.node, g.vmid, storage, mode, notes, s), () => void this.loadBackups());
  }

  /** Restores a backup to `vmid` on `node`; overwriting an existing guest needs its id typed. */
  askRestore(f: PveBackupFile, node: string, vmid: number, storage: string, start: boolean) {
    const id = this.hostId;
    if (id === null) return;
    const existing = this.overview?.guests.find((g) => g.vmid === vmid);
    const run = () =>
      this.startTask(`Restore ${f.vmid} to ${vmid}`, node, (s) => api.pveRestore(id, node, f.kind, vmid, f.volid, storage, !!existing, start, s), () => void this.loadBackups());
    if (!existing) {
      void run().catch(() => {});
      return;
    }
    this.confirm = {
      title: "RESTORE OVER A GUEST",
      body: `${existing.name} (${vmid}) on ${existing.node} exists. Restoring over it replaces its configuration and disks with the backup from ${new Date((f.time ?? 0) * 1000).toLocaleString()}. Anything it has now is lost.${existing.status === "running" ? " Stop the guest first: Proxmox refuses to restore over a running one." : ""}`,
      button: `Overwrite ${vmid}`,
      typed: String(vmid),
      run,
    };
  }

  async protect(f: PveBackupFile, on: boolean) {
    const id = this.hostId;
    const node = this.overview?.nodes.find((n) => n.local)?.name ?? f.storage;
    if (id === null) return;
    try {
      await this.sudo(on ? "protect a backup" : "unprotect a backup", (s) => api.pveBackupProtect(id, node, f.volid, on, s));
      await this.loadBackups();
    } catch (e) {
      this.notice = { text: `${on ? "Protect" : "Unprotect"}: ${errorText(e)}`, tone: "err" };
    }
  }

  askDeleteBackup(f: PveBackupFile) {
    const id = this.hostId;
    if (id === null) return;
    this.confirm = {
      title: "DELETE BACKUP",
      body: `Delete the backup of ${f.vmid} from ${new Date((f.time ?? 0) * 1000).toLocaleString()} on ${f.storage}? The file is removed from the storage: ${f.volid}`,
      button: "Delete backup",
      run: async () => {
        await this.sudo("delete a backup", (s) => api.pveBackupDelete(id, f.volid, s));
        this.notice = { text: "Backup deleted.", tone: "ok" };
        await this.loadBackups();
      },
    };
  }

  // ---- part 3: create, clone, templates, ISOs ----

  /** The node this host is (where new guests and downloads go). */
  get localNode(): string | null {
    return this.overview?.nodes.find((n) => n.local)?.name ?? this.overview?.nodes[0]?.name ?? null;
  }

  private storagesWith(content: string, node: string): string[] {
    return [...new Set((this.overview?.storages ?? []).filter((s) => s.node === node && s.status === "available" && s.content.includes(content)).map((s) => s.storage))];
  }

  async loadCreateInfo() {
    const id = this.hostId;
    const node = this.localNode;
    if (id === null || !node) return;
    try {
      this.createInfo = await this.sudo("read bridges and ISOs", (s) => api.pveCreateInfo(id, node, this.storagesWith("iso", node), this.storagesWith("vztmpl", node), s));
      this.createInfoError = null;
    } catch (e) {
      this.createInfoError = errorText(e);
    }
  }

  open(make: PveMake) {
    this.make = make;
    void this.loadCreateInfo();
    if (make.kind === "appliance" && !this.appliances) void this.loadAppliances();
  }

  async loadAppliances() {
    const id = this.hostId;
    const node = this.localNode;
    if (id === null || !node) return;
    try {
      this.appliances = await this.sudo("list container templates", (s) => api.pveAppliances(id, node, s));
    } catch (e) {
      this.createInfoError = errorText(e);
    }
  }

  async createVm(spec: PveVmSpec) {
    const id = this.hostId;
    if (id === null) return;
    await this.startTask(`Create VM ${spec.name}`, spec.node, (s) => api.pveVmCreate(id, spec, s));
    this.make = null;
  }

  async createCt(spec: PveCtSpec) {
    const id = this.hostId;
    if (id === null) return;
    await this.startTask(`Create CT ${spec.hostname}`, spec.node, (s) => api.pveCtCreate(id, spec, s));
    this.make = null;
  }

  async clone(spec: PveCloneSpec) {
    const id = this.hostId;
    if (id === null) return;
    await this.startTask(`Clone ${spec.vmid} to ${spec.newid}`, spec.node, (s) => api.pveClone(id, spec, s));
    this.make = null;
  }

  // ---- sandbox clones (M12c) ----

  /** Notes from the last sandbox made, shown on the SANDBOX tab. */
  sandboxNotes = $state<{ vmid: number; notes: string[] } | null>(null);

  /** Clones, then cuts the copy off and marks it once the clone task ends. */
  async makeSandbox(spec: PveSandboxSpec) {
    const id = this.hostId;
    if (id === null) return;
    await this.startTask(`Sandbox ${spec.name}`, spec.node, (s) => api.pveSandboxCreate(id, spec, s), (d) => void this.isolateSandbox(id, spec, d));
    this.make = null;
    this.tab = "sandbox";
  }

  private async isolateSandbox(id: number, spec: PveSandboxSpec, d: PveTaskDetail) {
    if (d.exitstatus && d.exitstatus !== "OK") {
      this.notice = { text: `Sandbox ${spec.name}: the clone failed (${d.exitstatus}).`, tone: "err" };
      return;
    }
    try {
      const done = await this.sudo("cut the sandbox off", (s) => api.pveSandboxIsolate(id, spec, s));
      this.sandboxNotes = { vmid: spec.newid, notes: done.notes };
      this.notice = { text: `Sandbox ${spec.name} (${spec.newid}) is ready and cut off${done.upid ? ", starting" : ""}.`, tone: "ok" };
      if (done.upid) void this.openTask(spec.node, done.upid, `Start ${spec.name}`);
      else void this.load();
    } catch (e) {
      this.notice = { text: `Sandbox ${spec.newid} was cloned but not cut off: ${errorText(e)}. Don't start it; destroy it from GUESTS.`, tone: "err" };
      void this.load();
    }
  }

  async extendSandbox(g: PveGuest, hours: number) {
    const id = this.hostId;
    if (id === null) return;
    this.busy = [...this.busy, g.vmid];
    try {
      await this.sudo("change the sandbox expiry", (s) => api.pveSandboxExtend(id, g.node, g.kind, g.vmid, hours, s));
      this.notice = { text: hours ? `${g.name} now expires in ${hours} h.` : `${g.name} no longer expires.`, tone: "ok" };
      void this.load();
    } catch (e) {
      this.notice = { text: `${g.name}: ${errorText(e)}`, tone: "err" };
    } finally {
      this.busy = this.busy.filter((v) => v !== g.vmid);
    }
  }

  askKeepSandbox(g: PveGuest) {
    const id = this.hostId;
    if (id === null) return;
    this.confirm = {
      title: "KEEP SANDBOX",
      body: `Keep ${g.name} (${g.vmid}) as an ordinary guest? Its expiry timer and sandbox tags go. Its network stays cut off; reconnect it under Hardware when you're sure it won't clash with the original (same IP, same hostname).`,
      button: `Keep ${g.vmid}`,
      run: async () => {
        await this.sudo("keep the sandbox", (s) => api.pveSandboxKeep(id, g.node, g.kind, g.vmid, s));
        this.notice = { text: `${g.name} is now an ordinary guest.`, tone: "ok" };
        void this.load();
      },
    };
  }

  askDestroySandbox(g: PveGuest) {
    const id = this.hostId;
    if (id === null) return;
    this.confirm = {
      title: "DESTROY SANDBOX",
      body: `Stop and destroy sandbox ${g.name} (${g.vmid}) on ${g.node} now? Its disks are deleted. The guest it was copied from isn't touched.`,
      button: `Destroy ${g.vmid}`,
      run: () => this.startTask(`Destroy ${g.name || g.vmid}`, g.node, (s) => api.pveSandboxDestroy(id, g.node, g.kind, g.vmid, s)),
    };
  }

  askTemplate(g: PveGuest) {
    const id = this.hostId;
    if (id === null) return;
    this.confirm = {
      title: "CONVERT TO TEMPLATE",
      body: `Turn ${g.name} (${g.vmid}) on ${g.node} into a template? A template can't be started or changed back; you clone it to make new guests. Its disks become read-only bases for linked clones.`,
      button: `Make ${g.name} a template`,
      run: () => this.startTask(`Template ${g.name}`, g.node, (s) => api.pveTemplate(id, g.node, g.kind, g.vmid, s)),
    };
  }

  askDestroy(g: PveGuest) {
    const id = this.hostId;
    if (id === null) return;
    this.confirm = {
      title: g.template ? "DESTROY TEMPLATE" : "DESTROY GUEST",
      body: `Destroy ${g.name} (${g.vmid}) on ${g.node}? Its configuration and all of its disks are deleted, and it is removed from backup jobs and HA. Backups on backup storages stay.`,
      button: `Destroy ${g.vmid}`,
      typed: g.name || String(g.vmid),
      run: () => this.startTask(`Destroy ${g.name || g.vmid}`, g.node, (s) => api.pveDestroy(id, g.node, g.kind, g.vmid, s)),
    };
  }

  async downloadIso(storage: string, url: string, filename: string, sha256: string | null) {
    const id = this.hostId;
    const node = this.localNode;
    if (id === null || !node) return;
    await this.startTask(`Download ${filename}`, node, (s) => api.pveIsoDownload(id, node, storage, url, filename, sha256, s), () => void this.loadCreateInfo());
    this.make = null;
  }

  async downloadAppliance(storage: string, template: string) {
    const id = this.hostId;
    const node = this.localNode;
    if (id === null || !node) return;
    await this.startTask(`Download ${template}`, node, (s) => api.pveApplianceDownload(id, node, storage, template, s), () => void this.loadCreateInfo());
  }

  storagesFor(content: string): string[] {
    const node = this.localNode;
    return node ? this.storagesWith(content, node) : [];
  }

  // ---- part 4: hardware and options ----

  /** Switches the guest window's tab, reading what the tab needs. */
  guestTab(tab: GuestDetail["tab"]) {
    if (!this.detail) return;
    this.detail = { ...this.detail, tab };
    if (tab === "hardware" && !this.detail.config) void this.loadConfig();
  }

  async loadConfig() {
    const d = this.detail;
    const id = this.hostId;
    if (!d || id === null) return;
    const g = d.guest;
    try {
      const config = await this.sudo("read the guest's hardware", (s) => api.pveConfig(id, g.node, g.kind, g.vmid, s));
      if (this.detail?.guest.vmid === g.vmid) this.detail = { ...this.detail, config, configError: null };
    } catch (e) {
      if (this.detail?.guest.vmid === g.vmid) this.detail = { ...this.detail, configError: errorText(e) };
    }
  }

  async loadDevices() {
    const d = this.detail;
    const id = this.hostId;
    if (!d || id === null) return;
    try {
      const devices = await this.sudo("list PCI and USB devices", (s) => api.pveHostDevices(id, d.guest.node, s));
      if (this.detail?.guest.vmid === d.guest.vmid) this.detail = { ...this.detail, devices };
    } catch (e) {
      this.notice = { text: `Devices: ${errorText(e)}`, tone: "err" };
    }
  }

  /** Sends only the given changes, with the digest the config was read at, then rereads it. */
  async applyConfig(what: string, set: PveSetting[], remove: string[] = []) {
    const d = this.detail;
    const id = this.hostId;
    if (!d?.config || id === null || (!set.length && !remove.length)) return;
    const g = d.guest;
    try {
      await this.sudo(what.toLowerCase(), (s) => api.pveConfigSet(id, g.node, g.kind, g.vmid, d.config!.digest, set, remove, s));
      this.notice = { text: `${what}: done.`, tone: "ok" };
    } catch (e) {
      this.notice = { text: `${what}: ${errorText(e)}`, tone: "err" };
      throw e;
    } finally {
      await this.loadConfig();
      void this.load();
    }
  }

  /** A change that removes or detaches something: named and confirmed first (rule 5). */
  askConfig(c: { title: string; body: string; button: string; typed?: string; set?: PveSetting[]; remove: string[] }) {
    this.confirm = { title: c.title, body: c.body, button: c.button, typed: c.typed, run: () => this.applyConfig(c.button, c.set ?? [], c.remove) };
  }

  async growDisk(disk: string, gb: number) {
    const d = this.detail;
    const id = this.hostId;
    if (!d || id === null) return;
    const g = d.guest;
    const what = `Grow ${disk} of ${g.name || g.vmid} by ${gb} GB`;
    try {
      const upid = await this.sudo(what.toLowerCase(), (s) => api.pveDiskResize(id, g.node, g.kind, g.vmid, disk, gb, s));
      this.notice = { text: `${what}: ${upid ? "started" : "done"}.`, tone: "ok" };
      if (upid) void this.openTask(g.node, upid, what, () => void this.loadConfig());
      else await this.loadConfig();
    } catch (e) {
      this.notice = { text: `${what}: ${errorText(e)}`, tone: "err" };
      throw e;
    }
  }

  // ---- part 5: node network ----

  async loadNetwork() {
    const id = this.hostId;
    const node = this.localNode;
    if (id === null || !node) return;
    try {
      this.network = await this.sudo("read the node's network", (s) => api.pveNetwork(id, node, s));
      this.networkError = null;
    } catch (e) {
      this.networkError = errorText(e);
    }
  }

  /** Stages a change; nothing reaches the node's live network until apply. */
  async saveIface(spec: PveIfaceSpec, create: boolean) {
    const id = this.hostId;
    const node = this.localNode;
    if (id === null || !node) return;
    try {
      await this.sudo(`stage ${spec.iface}`, (s) => api.pveIfaceSave(id, node, spec, create, s));
      this.notice = { text: `${spec.iface}: staged. Apply to make it live.`, tone: "ok" };
    } catch (e) {
      this.notice = { text: `${spec.iface}: ${errorText(e)}`, tone: "err" };
      throw e;
    } finally {
      await this.loadNetwork();
    }
  }

  askDeleteIface(iface: string, carriesAddress: boolean) {
    const id = this.hostId;
    const node = this.localNode;
    if (id === null || !node) return;
    this.confirm = {
      title: "REMOVE INTERFACE",
      body: `Stage removing ${iface} on ${node}? Nothing changes until you apply.${carriesAddress ? ` ${iface} carries an address of the node: applying this may cut the node off.` : ""} Guests attached to it lose their network when it's applied.`,
      button: `Remove ${iface}`,
      run: async () => {
        await this.sudo(`stage removing ${iface}`, (s) => api.pveIfaceDelete(id, node, iface, s));
        await this.loadNetwork();
      },
    };
  }

  async revertNetwork() {
    const id = this.hostId;
    const node = this.localNode;
    if (id === null || !node) return;
    try {
      await this.sudo("discard staged network changes", (s) => api.pveNetworkRevert(id, node, s));
      this.notice = { text: "Staged network changes discarded.", tone: "ok" };
    } catch (e) {
      this.notice = { text: `Revert: ${errorText(e)}`, tone: "err" };
    }
    await this.loadNetwork();
  }

  askApplyNetwork() {
    const id = this.hostId;
    const node = this.localNode;
    if (id === null || !node) return;
    this.confirm = {
      title: "APPLY NETWORK CHANGES",
      body: `Apply the staged changes to ${node}'s live network (ifreload)? A rollback is armed on ${node} first: unless you press Keep within 90 seconds, it restores the current config by itself, so a change that cuts this computer off undoes itself.`,
      button: `Apply on ${node}`,
      run: async () => {
        const r = await this.sudo("apply network changes", (s) => api.pveNetworkApply(id, node, s));
        this.netApply = { token: r.token, node, until: Date.now() + r.secs * 1000, upid: r.upid };
        this.notice = { text: `Network applied on ${node}. Keep it within ${r.secs} s or it rolls back.`, tone: "ok" };
        if (r.upid) void this.openTask(node, r.upid, `Apply network on ${node}`, () => void this.loadNetwork());
        else void this.loadNetwork();
      },
    };
  }

  async settleNetwork(keep: boolean) {
    const a = this.netApply;
    const id = this.hostId;
    if (!a || id === null) return;
    try {
      await this.sudo(keep ? "keep the network change" : "roll the network back", (s) => api.pveNetworkSettle(id, a.token, keep, s));
      this.netApply = null;
      this.notice = { text: keep ? `Network change on ${a.node} kept.` : `Network on ${a.node} rolled back.`, tone: "ok" };
    } catch (e) {
      this.notice = { text: `${keep ? "Keep" : "Roll back"}: ${errorText(e)}. If the node can't be reached, it rolls back by itself.`, tone: "err" };
    }
    await this.loadNetwork();
  }

  // ---- part 6: firewall ----

  async loadFirewall(scope: PveFwScope): Promise<PveFirewall> {
    const id = this.hostId;
    if (id === null) throw new Error("no host");
    return this.sudo("read the Proxmox firewall", (s) => api.pveFirewall(id, scope, s));
  }

  /** Runs a firewall change; host-level ones join the armed session. */
  async fwChange(what: string, f: (id: number, local: string, token: string | null, s: Parameters<typeof api.pveOverview>[1]) => Promise<PveFwRun>) {
    const id = this.hostId;
    const local = this.localNode;
    if (id === null || !local) return;
    try {
      const r = await this.sudo(what.toLowerCase(), (s) => f(id, local, this.fwSession?.token ?? null, s));
      if (r.armed) this.fwSession = { token: r.token, until: Date.now() + r.secs * 1000 };
      this.notice = { text: `${what}: done.${r.armed ? " Keep the firewall changes within 90 s or they roll back." : ""}`, tone: "ok" };
    } catch (e) {
      this.notice = { text: `${what}: ${errorText(e)}`, tone: "err" };
      throw e;
    }
  }

  saveRule(scope: PveFwScope, pos: number | null, digest: string, rule: PveFwRuleSpec) {
    return this.fwChange(pos === null ? "Add rule" : `Change rule ${pos}`, (id, local, token, s) => api.pveFwRuleSave(id, scope, local, token, pos, digest, rule, s));
  }

  moveRule(scope: PveFwScope, pos: number, to: number, digest: string) {
    return this.fwChange(`Move rule ${pos}`, (id, local, token, s) => api.pveFwRuleMove(id, scope, local, token, pos, to, digest, s));
  }

  askDeleteRule(scope: PveFwScope, pos: number, label: string, digest: string, after: () => void) {
    this.confirm = {
      title: "DELETE FIREWALL RULE",
      body: `Delete rule ${pos} (${label}) from the ${scopeName(scope)} firewall?`,
      button: `Delete rule ${pos}`,
      run: async () => {
        await this.fwChange(`Delete rule ${pos}`, (id, local, token, s) => api.pveFwRuleDelete(id, scope, local, token, pos, digest, s));
        after();
      },
    };
  }

  fwOptions(scope: PveFwScope, enable: boolean, policyIn: string, policyOut: string, digest: string) {
    return this.fwChange(`${scopeName(scope)} firewall ${enable ? "on" : "off"}`, (id, local, token, s) => api.pveFwOptions(id, scope, local, token, enable, policyIn, policyOut, digest, s));
  }

  fwObject(op: string, name: string, cidr = "", comment = "") {
    return this.fwChange(`${op.replace("-", " ")} ${name}`, (id, local, token, s) => api.pveFwObject(id, local, token, op, name, cidr, comment, s));
  }

  async settleFirewall(keep: boolean) {
    const f = this.fwSession;
    const id = this.hostId;
    if (!f || id === null) return;
    try {
      await this.sudo(keep ? "keep the firewall changes" : "roll the firewall back", (s) => api.pveFwSettle(id, f.token, keep, s));
      this.fwSession = null;
      this.notice = { text: keep ? "Firewall changes kept." : "Firewall rolled back.", tone: "ok" };
    } catch (e) {
      this.notice = { text: `${keep ? "Keep" : "Roll back"}: ${errorText(e)}. If the node can't be reached, it rolls back by itself.`, tone: "err" };
    }
  }

  async runConfirm() {
    const c = this.confirm;
    if (!c) return;
    this.confirm = null;
    try {
      await c.run();
      sound.play("ok");
    } catch (e) {
      sound.play("fail");
      this.notice = { text: `${c.button}: ${errorText(e)}`, tone: "err" };
    }
  }

  /** Hides the task window; a running task with follow-up work keeps being followed. */
  closeTask() {
    const t = this.task;
    if (t && t.onDone && (!t.detail || t.detail.running)) {
      this.task = { ...t, hidden: true };
      return;
    }
    this.stopFollowing();
  }

  private stopFollowing() {
    if (this.follow) clearTimeout(this.follow);
    this.follow = null;
    this.task = null;
  }
}

export const proxmox = new ProxmoxStore();
