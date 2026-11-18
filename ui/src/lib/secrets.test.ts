import { describe, expect, it } from "vitest";
import type { SecretItem } from "./api";
import { ageText, dueText, fingerprintOf, manualId, policyLabel, summary } from "./secrets";

const NOW = 1_800_000_000;
const DAY = 86_400;
const mk = (p: Partial<SecretItem>): SecretItem => ({ key: "k", kind: "sshKey", name: "n", place: "", hostId: null, rotatedAt: null, policyDays: 365, dueAt: null, state: "unknown", note: "", ...p });

describe("secrets", () => {
  it("words due dates", () => {
    expect(dueText(mk({ dueAt: NOW + 20 * DAY, state: "dueSoon" }), NOW)).toBe("due in 20 days");
    expect(dueText(mk({ dueAt: NOW - 90 * DAY, state: "overdue" }), NOW)).toBe("3 months overdue");
    expect(dueText(mk({ kind: "expiry", dueAt: NOW + 9 * DAY }), NOW)).toBe("expires in 9 days");
    expect(dueText(mk({ state: "noPolicy", policyDays: null }), NOW)).toBe("no rotation policy");
    expect(dueText(mk({}), NOW)).toBe("mark it rotated to start the clock");
  });
  it("words ages and policies", () => {
    expect(ageText(mk({ rotatedAt: NOW - 400 * DAY }), NOW)).toBe("rotated 13 months ago");
    expect(ageText(mk({}), NOW)).toBe("no date yet");
    expect([30, 365, 730, 0].map(policyLabel)).toEqual(["30 days", "1 year", "2 years", "never"]);
  });
  it("sums and parses keys", () => {
    const s = summary([mk({ state: "overdue" }), mk({ state: "dueSoon" }), mk({ state: "ok" })]);
    expect(s).toEqual({ total: 3, overdue: 1, soon: 1, unknown: 0 });
    expect(fingerprintOf("ssh:SHA256:abc")).toBe("SHA256:abc");
    expect(fingerprintOf("vault:1")).toBeNull();
    expect(manualId("manual:7")).toBe(7);
  });
});
