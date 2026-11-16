/**
 * Browser-only stand-in for UPS watches (M10s): one UPS on mains and one on
 * battery. Open the preview with `?upsdemo` to see a running shutdown countdown.
 */
import type { UpsCountdown, UpsInput_Deserialize as UpsInput, UpsReading_Serialize as UpsReading, UpsStatus_Serialize as UpsStatus, UpsWatch_Serialize as UpsWatch } from "./bindings";

const now = () => Math.floor(Date.now() / 1000);

let watches: UpsWatch[] = [
  { id: 1, label: "Rack UPS", target: "ups@localhost", hostId: 1, groupId: 1, shutdown: true, onBatterySecs: 300, lowRuntimeSecs: 300, countdownSecs: 120, enabled: true },
  { id: 2, label: "Desk UPS", target: "desk", hostId: 2, groupId: null, shutdown: false, onBatterySecs: 0, lowRuntimeSecs: 180, countdownSecs: 60, enabled: true },
];
let nextId = 3;

const readings: Record<number, UpsReading> = {
  1: { status: ["OL", "CHRG"], onBattery: false, lowBattery: false, replaceBattery: false, charge: 100, runtime: 2460, load: 23, inputVoltage: 231, model: "Back-UPS 1500", error: null },
  2: { status: ["OB", "DISCHRG"], onBattery: true, lowBattery: false, replaceBattery: true, charge: 64, runtime: 610, load: 41, inputVoltage: 0, model: "Eco 650", error: null },
};

let countdown: UpsCountdown | null =
  typeof location !== "undefined" && location.search.includes("upsdemo")
    ? { watchId: 1, label: "Rack UPS", reason: "on battery for 5 minutes", deadline: now() + 95, targets: ["server02", "server01"] }
    : null;
let onChange: (() => void) | null = null;

export function mockUpsCountdown(): UpsCountdown | null {
  return countdown;
}

export const mockUps = {
  /** mockAlerts re-sends its snapshot through this. */
  watch(fn: () => void) {
    onChange = fn;
  },
  async list(): Promise<UpsWatch[]> {
    return [...watches];
  },
  async save(input: UpsInput): Promise<UpsWatch> {
    const w: UpsWatch = {
      id: input.id ?? nextId++,
      label: input.label?.trim() || input.target || "UPS",
      target: input.target ?? "ups",
      hostId: input.hostId ?? 1,
      groupId: input.groupId ?? null,
      shutdown: input.shutdown ?? false,
      onBatterySecs: input.onBatterySecs ?? 300,
      lowRuntimeSecs: input.lowRuntimeSecs ?? 300,
      countdownSecs: input.countdownSecs ?? 120,
      enabled: input.enabled ?? true,
    };
    watches = watches.some((x) => x.id === w.id) ? watches.map((x) => (x.id === w.id ? w : x)) : [...watches, w];
    return w;
  },
  async delete(id: number): Promise<null> {
    watches = watches.filter((w) => w.id !== id);
    return null;
  },
  async status(): Promise<UpsStatus[]> {
    return watches.map((w) => ({
      watch: w,
      reading: readings[w.id] ?? null,
      readAt: readings[w.id] ? now() - 12 : null,
      onBatterySince: readings[w.id]?.onBattery ? now() - 140 : null,
    }));
  },
  async readNow(_hostId: number, target: string): Promise<UpsReading> {
    if (!/^[A-Za-z0-9_.-]+(@[A-Za-z0-9_.:-]+)?$/.test(target)) throw new Error("a NUT name like ups or ups@localhost");
    if (target.startsWith("x")) return { status: [], onBattery: false, lowBattery: false, replaceBattery: false, charge: null, runtime: null, load: null, inputVoltage: null, model: null, error: "Unknown UPS" };
    return { ...readings[1] };
  },
  async cancel(): Promise<null> {
    countdown = null;
    onChange?.();
    return null;
  },
};
