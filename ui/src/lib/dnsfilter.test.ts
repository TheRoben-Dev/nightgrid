import { describe, expect, it } from "vitest";
import { dnsSummary, duration, isIp, listName, sortLists, validRecordDomain } from "./dnsfilter";
import type { DnsList, DnsStatus } from "./api";

const base: DnsStatus = { version: null, blocking: true, resumesIn: null, queries: 48213, blocked: 6120, percentBlocked: 12.69, clients: 3, onLists: 10, topBlocked: [], lists: [], records: [] };
const list = (p: Partial<DnsList>): DnsList => ({ url: "https://a.example/x.txt", name: "", enabled: true, allow: false, entries: null, updated: null, ...p });

describe("dns filter helpers", () => {
  it("summarises", () => {
    expect(dnsSummary(base)).toBe("48,213 queries · 12.7% blocked");
    expect(dnsSummary({ ...base, blocking: false, resumesIn: 299 })).toBe("BLOCKING PAUSED · back in 5 min");
    expect(dnsSummary({ ...base, blocking: false })).toBe("BLOCKING OFF");
  });
  it("formats durations", () => {
    expect([duration(45), duration(300), duration(3600), duration(5400)]).toEqual(["45 s", "5 min", "1 h", "1 h 30 min"]);
  });
  it("names and sorts lists", () => {
    expect(listName(list({ url: "https://raw.example.com/hosts/master/hosts?x=1" }))).toBe("hosts");
    expect(listName(list({ name: "Steven Black" }))).toBe("Steven Black");
    const s = sortLists([list({ name: "b", allow: true }), list({ name: "z" }), list({ name: "a" })]);
    expect(s.map((l) => l.name)).toEqual(["a", "z", "b"]);
  });
  it("checks records", () => {
    expect(validRecordDomain("nas.lan", false)).toBe(true);
    expect(validRecordDomain("*.apps.lan", false)).toBe(false);
    expect(validRecordDomain("*.apps.lan", true)).toBe(true);
    expect(validRecordDomain("a b", true)).toBe(false);
    expect([isIp("192.168.1.10"), isIp("fd00::5"), isIp("nas.lan")]).toEqual([true, true, false]);
  });
});
