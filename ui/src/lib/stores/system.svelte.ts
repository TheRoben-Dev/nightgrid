import {
  api,
  errorText,
  systemError,
  type ConnectError,
  type DisksReport,
  type Schedules,
  type SecurityReport,
  type Health,
  type JournalEntry,
  type JournalEvent,
  type PromptKind,
  type SudoInput,
  type SystemInfo,
  type Unit,
  type UnitAction,
  type UpdatesReport,
  type UpgradeOp,
} from "../api";
import { backups } from "./backups.svelte";
import { hosts } from "./hosts.svelte";

export type SystemTab = "units" | "journal" | "schedules" | "security" | "updates" | "disks" | "backups";
export type UnitFilter = "all" | "running" | "failed" | "inactive";

/** Journal rows kept in memory; older ones scroll off. */
export const MAX_JOURNAL = 5000;

export const PRIORITIES = ["EMERG", "ALERT", "CRIT", "ERR", "WARN", "NOTICE", "INFO", "DEBUG"] as const;
export const prioLabel = (p: number): string => PRIORITIES[p] ?? "INFO";
/** Color class for a priority: 0–3 bad, 4 warn, 5 notice, 6 info, 7 debug. */
export const prioTone = (p: number): string => (p <= 3 ? "bad" : p === 4 ? "warn" : p === 5 ? "note" : p === 7 ? "dim" : "");

export const UNIT_ACTIONS: UnitAction[] = ["start", "stop", "restart", "reload", "enable", "disable"];
/** Stop / restart / disable need a confirmation naming the unit (rule 5). */
export const isDestructiveUnit = (a: UnitAction): boolean => a === "stop" || a === "restart" || a === "disable";
export const actionVerb = (a: UnitAction): string => a[0].toUpperCase() + a.slice(1);

export const UNIT_CONSEQUENCE: Partial<Record<UnitAction, string>> = {
  stop: "The service stops now and stays stopped until started again (or until the next boot if it's enabled).",
  restart: "The service stops and starts again; its open connections are dropped.",
  disable: "The service no longer starts at boot. It keeps running for now.",
};

export type Tone = "ok" | "bad" | "warn" | "off";

export function unitTone(u: Unit): Tone {
  if (u.active === "failed") return "bad";
  if (u.active === "active") return "ok";
  if (["activating", "deactivating", "reloading", "refreshing"].includes(u.active)) return "warn";
  return "off";
}

export function filterUnits(units: Unit[], filter: UnitFilter, query: string): Unit[] {
  const q = query.trim().toLowerCase();
  return units.filter((u) => {
    if (filter === "running" && u.active !== "active") return false;
    if (filter === "failed" && u.active !== "failed") return false;
    if (filter === "inactive" && u.active !== "inactive") return false;
    return !q || u.name.toLowerCase().includes(q) || u.description.toLowerCase().includes(q);
  });
}

export function unitCounts(units: Unit[]): { running: number; failed: number; total: number } {
  return {
    running: units.filter((u) => u.active === "active" && u.sub === "running").length,
    failed: units.filter((u) => u.active === "failed").length,
    total: units.length,
  };
}

/** Client-side text filter over loaded journal rows. */
export function filterJournal(entries: JournalEntry[], query: string): JournalEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return entries;
  return entries.filter((e) => e.message.toLowerCase().includes(q) || (e.ident ?? "").toLowerCase().includes(q) || (e.unit ?? "").toLowerCase().includes(q));
}

/** "4y 307d" style age from power-on hours. */
export function formatHours(h: number | null | undefined): string {
  if (h == null) return "—";
  if (h < 48) return `${h} h`;
  const days = Math.floor(h / 24);
  return days >= 365 ? `${Math.floor(days / 365)}y ${days % 365}d` : `${days}d`;
}

export const HEALTH_WORD: Record<Health, string> = { ok: "HEALTHY", unknown: "UNKNOWN", warn: "WARNING", bad: "FAILING" };

export interface JournalForm {
  unit: string;
  /** Most verbose priority shown (0–7). */
  priority: number;
  since: string;
  until: string;
  lines: number;
  follow: boolean;
}

export const defaultJournalForm = (): JournalForm => ({ unit: "", priority: 6, since: "", until: "", lines: 500, follow: true });

export interface PackageRun {
  op: UpgradeOp;
  lines: string[];
  running: boolean;
  code: number | null;
  error: string | null;
}

class SystemStore {
  hostId = $state<number | null>(null);
  tab = $state<SystemTab>("units");
  info = $state<SystemInfo | null>(null);
  /** SSH prompt to answer before connecting (host key, password). */
  prompt = $state<ConnectError | null>(null);
  /** Per-tab error text (install hints included). */
  errors = $state<Partial<Record<SystemTab, string>>>({});
  notice = $state<{ text: string; tone: "ok" | "err" } | null>(null);

  /** Open sudo prompt: what it's for, and whether the last try failed. */
  sudoAsk = $state<{ reason: string; retry: boolean } | null>(null);
  private sudoResolve: ((v: SudoInput | null) => void) | null = null;

  units = $state<Unit[]>([]);
  unitsLoading = $state(false);
  unitsLoaded = $state(false);
  unitFilter = $state<UnitFilter>("all");
  unitQuery = $state("");
  selectedUnit = $state<string | null>(null);
  unitStatus = $state<string | null>(null);
  unitBusy = $state<string[]>([]);
  pendingUnit = $state<{ unit: string; action: UnitAction } | null>(null);

  journal = $state<JournalEntry[]>([]);
  journalForm = $state<JournalForm>(defaultJournalForm());
  journalQuery = $state("");
  journalRun = $state<number | null>(null);
  journalLimited = $state(false);
  journalElevated = $state(false);
  journalNotices = $state<string[]>([]);
  journalLoading = $state(false);

  updates = $state<UpdatesReport | null>(null);
  updatesLoading = $state(false);
  run = $state<PackageRun | null>(null);
  confirmUpgrade = $state(false);
  confirmReboot = $state(false);

  disks = $state<DisksReport | null>(null);
  disksLoading = $state(false);

  schedules = $state<Schedules | null>(null);
  schedulesLoading = $state(false);

  security = $state<SecurityReport | null>(null);
  securityLoading = $state(false);
  /** Port map over every host: reports (or errors) by host id. */
  portMap = $state<Record<number, SecurityReport | string>>({});
  portMapLoading = $state(false);

  private seq = 0;

  get host() {
    return this.hostId === null ? undefined : hosts.byId(this.hostId);
  }

  get filteredUnits(): Unit[] {
    return filterUnits(this.units, this.unitFilter, this.unitQuery);
  }

  get visibleJournal(): JournalEntry[] {
    return filterJournal(this.journal, this.journalQuery);
  }

  /** Whether root needs a typed password right now. */
  get sudoNeedsPassword(): boolean {
    return this.info?.sudo === "password" && !this.info.remembered;
  }

  select(id: number) {
    if (id === this.hostId) return;
    void this.stopJournal();
    this.seq++;
    this.hostId = id;
    this.info = null;
    this.prompt = null;
    this.errors = {};
    this.notice = null;
    this.units = [];
    this.unitsLoaded = false;
    this.selectedUnit = null;
    this.unitStatus = null;
    this.journal = [];
    this.journalLimited = false;
    this.journalElevated = false;
    this.journalNotices = [];
    this.updates = null;
    this.run = null;
    this.disks = null;
    this.schedules = null;
    this.security = null;
    void this.loadInfo();
    void this.openTab(this.tab);
  }

  async openTab(tab: SystemTab) {
    this.tab = tab;
    if (this.hostId === null || this.prompt) return;
    if (tab === "units" && !this.unitsLoaded) await this.loadUnits();
    else if (tab === "journal" && this.journalRun === null && !this.journal.length) await this.startJournal();
    else if (tab === "schedules" && !this.schedules) await this.loadSchedules();
    else if (tab === "security" && !this.security) await this.loadSecurity();
    else if (tab === "updates" && !this.updates) await this.checkUpdates();
    else if (tab === "backups" && !backups.loaded) await backups.load().then(() => backups.checkAll());
    else if (tab === "disks" && !this.disks && !this.sudoNeedsPassword && this.info) await this.loadDisks();
  }

  private fail(tab: SystemTab, e: unknown) {
    const { connect } = systemError(e);
    if (connect && connect.kind !== "network" && connect.kind !== "other") this.prompt = connect;
    else this.errors = { ...this.errors, [tab]: errorText(e) };
  }

  private clearError(tab: SystemTab) {
    if (this.errors[tab]) {
      const { [tab]: _e, ...rest } = this.errors;
      this.errors = rest;
    }
  }

  async loadInfo() {
    const id = this.hostId;
    if (id === null) return;
    try {
      const info = await api.systemInfo(id);
      if (id === this.hostId) this.info = info;
      if (this.tab === "disks" && !this.disks && !this.sudoNeedsPassword) void this.loadDisks();
    } catch (e) {
      if (id === this.hostId) this.fail(this.tab, e);
    }
  }

  // ---- sudo -------------------------------------------------------------

  askSudo(reason: string, retry: boolean): Promise<SudoInput | null> {
    this.sudoResolve?.(null);
    this.sudoAsk = { reason, retry };
    return new Promise((resolve) => (this.sudoResolve = resolve));
  }

  answerSudo(v: SudoInput | null) {
    const r = this.sudoResolve;
    this.sudoResolve = null;
    this.sudoAsk = null;
    r?.(v);
  }

  /**
   * Runs `f` without a password first (NOPASSWD, root or a remembered one),
   * and asks for the sudo password when the host wants it. Rejects with
   * "Cancelled" if the prompt is dismissed.
   */
  async withSudo<T>(reason: string, f: (sudo: SudoInput | null) => Promise<T>): Promise<T> {
    let sudo: SudoInput | null = null;
    for (;;) {
      try {
        const r = await f(sudo);
        if (sudo?.remember && this.info) this.info = { ...this.info, remembered: true };
        return r;
      } catch (e) {
        const { system } = systemError(e);
        if (system?.kind !== "sudoRequired") throw e;
        if (this.info?.remembered && !sudo) this.info = { ...this.info, remembered: false };
        sudo = await this.askSudo(reason, system.retry || sudo !== null);
        if (!sudo) throw new Error("Cancelled.", { cause: e });
      }
    }
  }

  async forgetSudo() {
    if (this.hostId === null) return;
    await api.systemSudoForget(this.hostId);
    if (this.info) this.info = { ...this.info, remembered: false };
    this.notice = { text: "Saved sudo password removed from the keychain.", tone: "ok" };
  }

  // ---- units ------------------------------------------------------------

  async loadUnits() {
    const id = this.hostId;
    if (id === null) return;
    const seq = this.seq;
    this.unitsLoading = true;
    try {
      const list = await api.systemUnits(id);
      if (seq !== this.seq) return;
      this.units = list;
      this.unitsLoaded = true;
      this.clearError("units");
    } catch (e) {
      if (seq === this.seq) this.fail("units", e);
    } finally {
      if (seq === this.seq) this.unitsLoading = false;
    }
  }

  async selectUnit(name: string | null) {
    this.selectedUnit = name;
    this.unitStatus = null;
    if (!name || this.hostId === null) return;
    const id = this.hostId;
    try {
      const text = await api.systemUnitStatus(id, name);
      if (this.selectedUnit === name && id === this.hostId) this.unitStatus = text;
    } catch (e) {
      if (this.selectedUnit === name) this.unitStatus = errorText(e);
    }
  }

  /** Destructive actions wait for confirmation; others run at once. */
  request(unit: string, action: UnitAction) {
    if (isDestructiveUnit(action)) this.pendingUnit = { unit, action };
    else void this.act(unit, action);
  }

  async act(unit: string, action: UnitAction) {
    const id = this.hostId;
    if (id === null) return;
    this.pendingUnit = null;
    this.unitBusy = [...this.unitBusy, unit];
    try {
      await this.withSudo(`${action} ${unit}`, (sudo) => api.systemUnitAction(id, unit, action, sudo));
      this.notice = { text: `${actionVerb(action)}: ${unit} done.`, tone: "ok" };
    } catch (e) {
      this.notice = { text: `${actionVerb(action)} ${unit}: ${errorText(e)}`, tone: "err" };
    } finally {
      this.unitBusy = this.unitBusy.filter((u) => u !== unit);
    }
    if (id === this.hostId) {
      await this.loadUnits();
      if (this.selectedUnit === unit) void this.selectUnit(unit);
    }
  }

  // ---- journal ----------------------------------------------------------

  /** Opens the journal tab filtered to `unit`. */
  openLogs(unit: string) {
    this.journalForm = { ...this.journalForm, unit };
    this.tab = "journal";
    void this.startJournal();
  }

  async startJournal(elevated = this.journalElevated) {
    const id = this.hostId;
    if (id === null) return;
    await this.stopJournal();
    const f = this.journalForm;
    this.journal = [];
    this.journalNotices = [];
    this.journalLimited = false;
    this.journalLoading = true;
    this.clearError("journal");
    const query = {
      unit: f.unit.trim() || null,
      priority: f.priority >= 7 ? null : f.priority,
      since: f.since.trim() || null,
      until: f.until.trim() || null,
      lines: f.lines,
      follow: f.follow && !f.until.trim(),
    };
    const on = (e: JournalEvent) => {
      if (id !== this.hostId) return;
      if (e.kind === "entries") {
        this.journalLoading = false;
        const next = this.journal.length + e.entries.length > MAX_JOURNAL ? [...this.journal, ...e.entries].slice(-MAX_JOURNAL) : [...this.journal, ...e.entries];
        this.journal = next;
      } else if (e.kind === "limited") {
        this.journalLimited = true;
      } else if (e.kind === "notice") {
        this.journalNotices = [...this.journalNotices, e.text].slice(-5);
      } else {
        this.journalLoading = false;
        this.journalRun = null;
        if (e.error) this.errors = { ...this.errors, journal: e.error };
      }
    };
    try {
      const run = elevated
        ? await this.withSudo("read the full journal", (sudo) => api.systemJournalStart(id, query, true, sudo, on))
        : await api.systemJournalStart(id, query, false, null, on);
      this.journalElevated = elevated;
      if (id === this.hostId) this.journalRun = run;
      else void api.systemStreamStop(run);
    } catch (e) {
      this.journalLoading = false;
      this.fail("journal", e);
    }
  }

  async stopJournal() {
    const run = this.journalRun;
    this.journalRun = null;
    if (run !== null) await api.systemStreamStop(run).catch(() => {});
  }

  /** Opens the journal filtered to one unit (e.g. the service a timer starts). */
  async showJournal(unit: string) {
    this.journalForm = { ...defaultJournalForm(), unit, priority: 7, since: "-7d", follow: false };
    this.journalQuery = "";
    this.tab = "journal";
    await this.startJournal();
  }

  // ---- schedules ----------------------------------------------------------

  async loadSchedules() {
    const id = this.hostId;
    if (id === null) return;
    const seq = this.seq;
    this.schedulesLoading = true;
    try {
      const s = await api.systemSchedules(id);
      if (seq !== this.seq) return;
      this.schedules = s;
      this.clearError("schedules");
    } catch (e) {
      if (seq === this.seq) this.fail("schedules", e);
    } finally {
      if (seq === this.seq) this.schedulesLoading = false;
    }
  }

  // ---- security -----------------------------------------------------------

  async loadSecurity() {
    const id = this.hostId;
    if (id === null) return;
    const seq = this.seq;
    this.securityLoading = true;
    try {
      const r = await api.systemSecurity(id);
      if (seq !== this.seq) return;
      this.security = r;
      this.portMap = { ...this.portMap, [id]: r };
      this.clearError("security");
    } catch (e) {
      if (seq === this.seq) this.fail("security", e);
    } finally {
      if (seq === this.seq) this.securityLoading = false;
    }
  }

  /**
   * Audits every host, a few at a time, for the cross-host port map. Hosts
   * that would need a password or a host key decision are skipped with a
   * note (no prompts from a bulk scan).
   */
  async scanAllPorts() {
    if (this.portMapLoading) return;
    this.portMapLoading = true;
    const queue = hosts.hosts.map((h) => h.id);
    const worker = async () => {
      for (let id = queue.shift(); id !== undefined; id = queue.shift()) {
        try {
          const r = await api.systemSecurity(id);
          this.portMap = { ...this.portMap, [id]: r };
        } catch (e) {
          const { connect } = systemError(e);
          this.portMap = { ...this.portMap, [id]: connect && connect.kind !== "network" && connect.kind !== "other" ? "Needs a login first (open a terminal on it)." : errorText(e) };
        }
      }
    };
    try {
      await Promise.all([worker(), worker(), worker()]);
    } finally {
      this.portMapLoading = false;
    }
  }

  // ---- packages -----------------------------------------------------------

  async checkUpdates() {
    const id = this.hostId;
    if (id === null) return;
    const seq = this.seq;
    this.updatesLoading = true;
    try {
      const r = await api.systemUpdates(id);
      if (seq !== this.seq) return;
      this.updates = r;
      this.clearError("updates");
    } catch (e) {
      if (seq === this.seq) this.fail("updates", e);
    } finally {
      if (seq === this.seq) this.updatesLoading = false;
    }
  }

  /** Refresh lists or install upgrades. Upgrades were confirmed first. */
  async runPackages(op: UpgradeOp) {
    const id = this.hostId;
    const manager = this.updates?.manager;
    if (id === null || !manager) return;
    this.confirmUpgrade = false;
    this.run = { op, lines: [], running: true, code: null, error: null };
    try {
      await this.withSudo(op === "refresh" ? "refresh package lists" : "install package upgrades", (sudo) =>
        api.systemUpgradeRun(id, manager, op, sudo, (e) => {
          if (!this.run || id !== this.hostId) return;
          if (e.kind === "lines") this.run = { ...this.run, lines: [...this.run.lines, ...e.lines].slice(-4000) };
          else {
            this.run = { ...this.run, running: false, code: e.code ?? null, error: e.error ?? null };
            void this.checkUpdates();
          }
        }),
      );
    } catch (e) {
      this.run = { op, lines: this.run?.lines ?? [], running: false, code: null, error: errorText(e) };
    }
  }

  async reboot() {
    const id = this.hostId;
    if (id === null) return;
    this.confirmReboot = false;
    try {
      await this.withSudo("reboot the host", (sudo) => api.systemReboot(id, sudo));
      this.notice = { text: `${this.host?.name ?? "Host"} is rebooting. Reconnect in a minute or two.`, tone: "ok" };
      this.updates = null;
      this.unitsLoaded = false;
      await this.stopJournal();
    } catch (e) {
      this.notice = { text: `Reboot: ${errorText(e)}`, tone: "err" };
    }
  }

  // ---- disks ------------------------------------------------------------

  async loadDisks() {
    const id = this.hostId;
    if (id === null) return;
    const seq = this.seq;
    this.disksLoading = true;
    try {
      const r = await this.withSudo("read SMART data", (sudo) => api.systemDisks(id, sudo));
      if (seq !== this.seq) return;
      this.disks = r;
      this.clearError("disks");
    } catch (e) {
      if (seq === this.seq) this.fail("disks", e);
    } finally {
      if (seq === this.seq) this.disksLoading = false;
    }
  }

  // ---- SSH prompts --------------------------------------------------------

  async acceptHostKey() {
    const p = this.prompt;
    if (!p || (p.kind !== "hostKeyUnknown" && p.kind !== "hostKeyChanged")) return;
    await api.sshTrustHostKey(p.host, p.port, p.key.openssh, p.kind === "hostKeyChanged");
    this.prompt = null;
    void this.loadInfo();
    await this.openTab(this.tab);
  }

  async provideSecret(secret: string, remember: boolean) {
    const p = this.prompt;
    if (!p || (p.kind !== "passwordRequired" && p.kind !== "passphraseRequired")) return;
    const kind: PromptKind = p.kind === "passwordRequired" ? "password" : "passphrase";
    await api.sshProvideSecret(p.target, kind, secret, remember);
    this.prompt = null;
    void this.loadInfo();
    await this.openTab(this.tab);
  }

  cancelPrompt() {
    this.prompt = null;
    this.errors = { ...this.errors, [this.tab]: "Connection cancelled." };
  }
}

export const system = new SystemStore();
