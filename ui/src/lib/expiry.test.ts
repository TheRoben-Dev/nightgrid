import { describe, expect, it } from "vitest";
import type { ExpiryPref, ProxyReport, Service, ServiceStatus, WatchItem } from "./api";
import { daysLeft, expiryCounts, expiryRows, fingerprintText, fromDateInput, leftText, toDateInput, toneOf, trustOf } from "./expiry";

const NOW = 1_800_000_000;
const DAY = 86_400;
const item = (id: number, days: number | null, warn = 30, extra: Partial<WatchItem> = {}): WatchItem => ({
  id,
  kind: "domain",
  name: `item ${id}`,
  hostId: null,
  target: "example.com",
  expiresAt: days === null ? null : NOW + days * DAY + 60,
  warnDays: warn,
  note: "",
  checkedAt: null,
  checkError: null,
  muted: false,
  ...extra,
});

describe("expiry watch", () => {
  it("counts days and tones", () => {
    expect(daysLeft(NOW + 10 * DAY + 5, NOW)).toBe(10);
    expect(daysLeft(NOW - 1, NOW)).toBe(-1);
    expect(daysLeft(null, NOW)).toBeNull();
    expect(toneOf(40, 30)).toBe("ok");
    expect(toneOf(20, 30)).toBe("warn");
    expect(toneOf(2, 30)).toBe("bad");
    expect(toneOf(null, 30)).toBe("unknown");
    expect(toneOf(null, 30, "file not found")).toBe("bad");
    expect(leftText(-3)).toBe("expired 3 days ago");
    expect(leftText(-1)).toBe("expired 1 day ago");
    expect(leftText(0)).toBe("today");
    expect(leftText(1)).toBe("tomorrow");
    expect(leftText(9)).toBe("in 9 days");
  });

  it("round-trips date inputs", () => {
    const t = fromDateInput("2027-03-01");
    expect(t).not.toBeNull();
    expect(toDateInput(t)).toBe("2027-03-01");
    expect(fromDateInput("soon")).toBeNull();
    expect(toDateInput(null)).toBe("");
  });

  it("merges items, service and proxy certificates, soonest first", () => {
    const services = [{ id: 7, name: "Media", url: "https://media.example.com/" } as Service];
    const status = { 7: { result: { tls: { expires: NOW + 5 * DAY, trusted: true, problem: null } } } } as unknown as Record<number, ServiceStatus>;
    const report = {
      routes: [
        { domains: ["media.example.com"], cert: { notAfter: NOW + 5 * DAY, issuer: null, error: null }, upstream: "media:80", container: "media" },
        { domains: ["wiki.example.com"], cert: { notAfter: NOW + 50 * DAY, issuer: null, error: null }, upstream: "wiki:80", container: "wiki" },
      ],
    } as unknown as ProxyReport;
    const rows = expiryRows(
      {
        items: [item(1, 100), item(2, null), item(3, 1, 30, { kind: "certFile", hostId: 4, target: "/etc/ssl/a.pem" })],
        services,
        status,
        proxy: { report, hostName: "docker1" },
        hostName: (id) => (id === 4 ? "lab" : ""),
        certDays: 14,
      },
      NOW,
    );
    expect(rows.map((r) => r.key)).toEqual(["w3", "s7", "pwiki.example.com", "w1", "w2"]);
    expect(rows[0].where).toBe("lab:/etc/ssl/a.pem");
    expect(rows[0].tone).toBe("bad");
    expect(rows[1].tone).toBe("warn");
    expect(rows[2].where).toBe("docker1 → wiki");
    expect(expiryCounts(rows)).toEqual({ bad: 1, warn: 1 });
  });

  it("applies edits to automatic rows: trust by fingerprint, mute, own date and days", () => {
    const FP = "ab".repeat(32);
    const services = [
      { id: 7, name: "Shoko", url: "https://shoko.lan/" },
      { id: 8, name: "Wiki", url: "https://wiki.lan/" },
    ] as Service[];
    const tls = (fingerprint: string) => ({ expires: NOW + 5000 * DAY, trusted: false, problem: "unknown issuer or self-signed", fingerprint });
    const status = { 7: { result: { tls: tls(FP) } }, 8: { result: { tls: tls("cd".repeat(32)) } } } as unknown as Record<number, ServiceStatus>;
    const base = { items: [item(1, 10, 30, { muted: true })], services, status, hostName: () => "", certDays: 14 };
    const before = expiryRows(base, NOW);
    expect(before.find((r) => r.key === "s7")?.error).toBe("unknown issuer or self-signed");
    expect(before.find((r) => r.key === "s7")?.tone).toBe("ok");
    expect(before.at(-1)?.key).toBe("w1");
    expect(before.at(-1)?.tone).toBe("muted");
    const prefs: ExpiryPref[] = [
      { target: { kind: "service", serviceId: 7 }, trustedFingerprint: FP, muted: false, expiresAt: NOW + 20 * DAY, warnDays: 30, note: "internal CA" },
      { target: { kind: "service", serviceId: 8 }, trustedFingerprint: FP, muted: true, expiresAt: null, warnDays: null, note: "" },
    ];
    const rows = expiryRows({ ...base, prefs }, NOW);
    const shoko = rows.find((r) => r.key === "s7")!;
    expect(shoko.trust).toBe("pinned");
    expect(shoko.error).toBeNull();
    expect(shoko.days).toBe(20);
    expect(shoko.certExpiresAt).toBe(NOW + 5000 * DAY);
    expect(shoko.tone).toBe("warn");
    // A different certificate than the trusted one warns again, even muted.
    const wiki = rows.find((r) => r.key === "s8")!;
    expect(wiki.trust).toBe("changed");
    expect(wiki.error).toContain("changed since you trusted it");
    expect(wiki.tone).toBe("warn");
    expect(expiryCounts(rows)).toEqual({ bad: 0, warn: 2 });
    expect(trustOf({ trusted: true }, null)).toBe("trusted");
    expect(fingerprintText("ab12cd")).toBe("AB:12:CD");
  });
});
