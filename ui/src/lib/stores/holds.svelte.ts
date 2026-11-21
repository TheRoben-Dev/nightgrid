/**
 * Update holds (D-195): what the user keeps out of updates. A held host is
 * skipped by batch upgrades and its pending count is silenced (for a while
 * or until released); a held image or stack is still checked but isn't
 * counted and Pull / Update leave it alone. Package holds live on the host
 * where it can hold them (apt-mark hold, dnf versionlock, hidden Windows
 * updates), else in Nightgrid. Nothing is held by default.
 */
import { api, errorText, type HoldInput, type PackageManager, type SudoInput, type UpdateHold, type UpdatesReport } from "../api";

/** Snooze choices for a host, in seconds (null = until released). */
export const HOLD_FOR: { label: string; secs: number | null }[] = [
  { label: "1 day", secs: 86_400 },
  { label: "1 week", secs: 7 * 86_400 },
  { label: "30 days", secs: 30 * 86_400 },
  { label: "Until I release it", secs: null },
];

/** "until Fri 21 Nov", "until released". */
export function holdUntilText(h: Pick<UpdateHold, "until">): string {
  if (h.until == null) return "until released";
  return `until ${new Intl.DateTimeFormat(undefined, { weekday: "short", day: "numeric", month: "short" }).format(h.until * 1000)}`;
}

/** Where a held package's hold lives: on the host or only in Nightgrid. */
export function packageHoldPlace(r: UpdatesReport | null | undefined, name: string, mine: UpdateHold[]): "host" | "nightgrid" | null {
  if (mine.some((h) => h.kind === "package" && h.target === name)) return "nightgrid";
  return r?.held.includes(name) ? "host" : null;
}

class HoldsStore {
  list = $state<UpdateHold[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);
  /** Something is being held or released (key → true). */
  busy = $state<Record<string, boolean>>({});
  /** Host whose "hold updates" dialog is open. */
  dialogHost = $state<number | null>(null);

  async load() {
    try {
      this.list = await api.updateHoldsList();
      this.loaded = true;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  ensure() {
    if (!this.loaded) void this.load();
  }

  forHost(hostId: number): UpdateHold[] {
    return this.list.filter((h) => h.hostId === hostId);
  }

  /** The host's own hold (snooze), if it is held. Ended snoozes don't count. */
  host(hostId: number): UpdateHold | null {
    const now = Date.now() / 1000;
    return this.list.find((h) => h.hostId === hostId && h.kind === "host" && (h.until == null || h.until > now)) ?? null;
  }

  stack(hostId: number, stack: string): UpdateHold | null {
    return this.list.find((h) => h.hostId === hostId && h.kind === "stack" && h.target === stack) ?? null;
  }

  image(hostId: number, stack: string, image: string): UpdateHold | null {
    return this.list.find((h) => h.hostId === hostId && h.kind === "image" && h.stack === stack && h.target === image) ?? null;
  }

  async save(input: HoldInput): Promise<string | null> {
    const key = `${input.hostId}/${input.kind}/${input.stack ?? ""}/${input.target ?? ""}`;
    this.busy = { ...this.busy, [key]: true };
    try {
      await api.updateHoldSave(input);
      await this.load();
      return null;
    } catch (e) {
      return (this.error = errorText(e));
    } finally {
      const { [key]: _done, ...rest } = this.busy;
      this.busy = rest;
    }
  }

  async release(id: number): Promise<string | null> {
    try {
      await api.updateHoldDelete(id);
      await this.load();
      return null;
    } catch (e) {
      return (this.error = errorText(e));
    }
  }

  holdHost(hostId: number, secs: number | null, note = ""): Promise<string | null> {
    return this.save({ hostId, kind: "host", until: secs == null ? null : Math.floor(Date.now() / 1000) + secs, note });
  }

  holdStack(hostId: number, stack: string): Promise<string | null> {
    return this.save({ hostId, kind: "stack", target: stack });
  }

  holdImage(hostId: number, stack: string, image: string, version: string | null): Promise<string | null> {
    return this.save({ hostId, kind: "image", target: image, stack, version });
  }

  /**
   * Holds or releases a package: on the host when its package manager can
   * (needs root: `sudo` as for upgrades), else in Nightgrid only.
   */
  async setPackage(hostId: number, report: UpdatesReport, name: string, hold: boolean, sudo: SudoInput | null, version: string | null = null): Promise<void> {
    const mine = this.list.find((h) => h.hostId === hostId && h.kind === "package" && h.target === name);
    if (!hold && mine) {
      await api.updateHoldDelete(mine.id);
      await this.load();
      return;
    }
    if (report.holdSupport === "host" && report.manager) {
      await api.systemPackageHold(hostId, report.manager as PackageManager, [name], hold, sudo);
      return;
    }
    if (report.holdSupport === "nightgrid" && hold) {
      await api.updateHoldSave({ hostId, kind: "package", target: name, version });
      await this.load();
      return;
    }
    throw new Error(report.holdSupport === "none" ? "This host can't hold single packages: hold the whole host instead." : "Nothing to release.");
  }
}

export const holds = new HoldsStore();
