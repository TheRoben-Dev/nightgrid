import { describe, expect, it } from "vitest";
import { blankUps, clock, needsTyped, remaining, triggerText, upsState, upsTone, upsWords, validTarget } from "./ups";
import type { UpsReading, UpsWatch } from "./api";

const reading = (p: Partial<UpsReading> = {}): UpsReading => ({ status: ["OL"], onBattery: false, lowBattery: false, replaceBattery: false, charge: 100, runtime: 1800, load: 20, inputVoltage: 230, model: null, realpower: null, realpowerNominal: null, powerNominal: null, error: null, ...p });
const watch = (p: Partial<UpsWatch> = {}): UpsWatch => ({ id: 1, label: "UPS", target: "ups", hostId: 1, groupId: 2, shutdown: true, onBatterySecs: 300, lowRuntimeSecs: 300, countdownSecs: 120, enabled: true, ...p });

describe("ups", () => {
  it("words durations like the Rust side", () => {
    expect(upsWords(45)).toBe("45 s");
    expect(upsWords(300)).toBe("5 min");
    expect(upsWords(3600)).toBe("1 h");
    expect(upsWords(5400)).toBe("1 h 30 min");
  });

  it("counts down to the deadline", () => {
    expect(remaining({ watchId: 1, label: "", reason: "", deadline: 1000, targets: [] }, 940_000)).toBe(60);
    expect(remaining({ watchId: 1, label: "", reason: "", deadline: 1000, targets: [] }, 2_000_000)).toBe(0);
    expect(clock(95)).toBe("1:35");
    expect(clock(5)).toBe("0:05");
  });

  it("tones and names the state", () => {
    expect(upsTone(null)).toBe("unknown");
    expect(upsTone(reading())).toBe("ok");
    expect(upsTone(reading({ replaceBattery: true }))).toBe("warn");
    expect(upsTone(reading({ onBattery: true, charge: 80 }))).toBe("warn");
    expect(upsTone(reading({ onBattery: true, charge: 20 }))).toBe("bad");
    expect(upsTone(reading({ lowBattery: true }))).toBe("bad");
    expect(upsState(reading({ error: "Unknown UPS" }))).toBe("ERROR");
    expect(upsState(reading({ onBattery: true }))).toBe("ON BATTERY");
  });

  it("says what a watch will do", () => {
    expect(triggerText(watch({ shutdown: false }))).toBe("Alerts only.");
    expect(triggerText(watch(), "rack")).toBe("Shuts down rack after 5 min on battery, or under 5 min of runtime, or on low battery, after a 2 min countdown you can cancel.");
    expect(triggerText(watch({ onBatterySecs: 0, lowRuntimeSecs: 0 }), "rack")).toBe("Shuts down rack on low battery, after a 2 min countdown you can cancel.");
  });

  it("checks NUT names like the Rust side", () => {
    expect(validTarget("ups")).toBe(true);
    expect(validTarget("ups@nas.lan:3493")).toBe(true);
    expect(validTarget("-x")).toBe(false);
    expect(validTarget("ups; rm -rf /")).toBe(false);
    expect(validTarget("")).toBe(false);
  });

  it("asks for the typed group name only when arming", () => {
    const form = { ...blankUps(1), shutdown: true, groupId: 2 };
    expect(needsTyped(null, form)).toBe(true);
    expect(needsTyped(watch({ shutdown: false }), form)).toBe(true);
    expect(needsTyped(watch({ groupId: 3 }), form)).toBe(true);
    expect(needsTyped(watch(), form)).toBe(false);
    expect(needsTyped(null, blankUps(1))).toBe(false);
  });
});
