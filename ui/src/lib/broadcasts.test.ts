import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { acknowledged, linkTarget, pending } from "./stores/broadcasts.svelte";
import { SOUNDS } from "./sound/sounds";
import type { Broadcast } from "./api";
// @ts-expect-error plain JS build script, no types
import { fromEnv, linkOk, parseTime, problems } from "../../../scripts/notices.mjs";

const b = (id: string, extra: Partial<Broadcast> = {}): Broadcast => ({
  id,
  level: "warn",
  title: id,
  body: "Do the thing.",
  link: null,
  os: [],
  minApp: null,
  maxApp: null,
  features: [],
  starts: null,
  expires: null,
  ...extra,
});

describe("transmission queue", () => {
  it("drops acknowledged ones and ones for features that are off", () => {
    const list = [b("a"), b("b", { features: ["tailnet"] }), b("c", { features: ["proxmox", "docker"] })];
    const on = (f: string) => f === "docker";
    expect(pending(list, [], on).map((x) => x.id)).toEqual(["a", "c"]);
    expect(pending(list, ["a"], () => true).map((x) => x.id)).toEqual(["b", "c"]);
  });

  it("remembers acknowledgements and forgets ids the feed dropped", () => {
    expect(acknowledged(["old", "a"], "b", [b("a"), b("b")])).toEqual(["a", "b"]);
    // Without a feed (it never loaded) nothing is forgotten.
    expect(acknowledged(["old"], "b", null)).toEqual(["old", "b"]);
    expect(acknowledged(["b"], "b", null)).toEqual(["b"]);
  });

  it("knows where buttons lead", () => {
    expect(linkTarget("kb:guides/tailnet")).toEqual({ kind: "kb", id: "guides/tailnet" });
    expect(linkTarget("view:fleet")).toEqual({ kind: "view", view: "fleet", tab: null });
    expect(linkTarget("view:settings/connections")).toEqual({ kind: "view", view: "settings", tab: "connections" });
    expect(linkTarget("view:settings/nope")).toEqual({ kind: "view", view: "settings", tab: null });
    expect(linkTarget("view:nowhere")).toBeNull();
    expect(linkTarget("https://example.org/x")).toEqual({ kind: "web", url: "https://example.org/x" });
    expect(linkTarget("javascript:alert(1)")).toBeNull();
  });

  it("has its own sound", () => {
    expect(SOUNDS.some((s) => s.id === "transmission" && s.category === "notices")).toBe(true);
  });
});

describe("notice feed script", () => {
  it("accepts the committed feed", () => {
    const src = JSON.parse(readFileSync("docs/notices/notices.json", "utf8"));
    expect(problems(src.notices)).toEqual([]);
  });

  it("checks fields like the app does", () => {
    const ok = { id: "a-1", level: "warn", title: "T", body: "b" };
    expect(problems([ok])).toEqual([]);
    const cases: [object, string][] = [
      [{ ...ok, id: "A_1" }, "lowercase"],
      [{ ...ok, level: "loud" }, "level"],
      [{ ...ok, title: "x".repeat(101) }, "title"],
      [{ ...ok, os: ["amiga"] }, "unknown os"],
      [{ ...ok, link: { label: "Go", to: "javascript:x" } }, "link must"],
      [{ ...ok, expires: "2026-02-30" }, "expires"],
      [{ ...ok, starts: "2026-12-02", expires: "2026-12-01" }, "expires before"],
      [{ ...ok, html: "<b>" }, "unknown field"],
    ];
    for (const [n, why] of cases) expect(problems([n]).join(" "), why).toContain(why);
    expect(problems([ok, ok]).join(" ")).toContain("twice");
  });

  it("parses times and links the same way as core/src/broadcast.rs", () => {
    expect(parseTime("2026-11-21")).toBe(1_795_219_200);
    expect(parseTime("2026-11-21T10:30:05Z")).toBe(1_795_219_200 + 37_805);
    expect(parseTime("2025-02-29")).toBeNull();
    for (const ok of ["kb:guides/tailnet-setup", "view:fleet", "view:fleet/disks", "https://tailscale.com/kb/1028"]) expect(linkOk(ok), ok).toBe(true);
    for (const bad of ["kb:guides", "view:Fleet", "http://x.y", "https://", "https://u@evil", "javascript:alert(1)"]) expect(linkOk(bad), bad).toBe(false);
  });

  it("builds a notice from workflow inputs", () => {
    const n = fromEnv({
      NOTICE_TITLE: "Tailscale keys expire Monday!",
      NOTICE_BODY: "Re-auth:\\n- open Settings",
      NOTICE_LEVEL: "warn",
      NOTICE_OS: "macOS, linux",
      NOTICE_LINK_TO: "view:settings/connections",
      NOTICE_LINK_LABEL: "",
      NOTICE_EXPIRES: "",
    });
    expect(n.id).toMatch(/^\d{4}-\d{2}-\d{2}-tailscale-keys-expire-monday$/);
    expect(n.body).toBe("Re-auth:\n- open Settings");
    expect(n.os).toEqual(["macos", "linux"]);
    expect(n.link).toEqual({ label: "Open", to: "view:settings/connections" });
    expect(n.expires).toBeUndefined();
    expect(problems([n])).toEqual([]);
  });
});
