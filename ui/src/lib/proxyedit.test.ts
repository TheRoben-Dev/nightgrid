import { describe, expect, it } from "vitest";
import { diffLines, mockProxyEdit, sites } from "./mockProxyEdit";
import { editSummary, splitDomains, targetKey } from "./proxyedit";

describe("proxy route editor", () => {
  it("splits domains and names changes", () => {
    expect(splitDomains(" a.example.com, b.example.com  c.lan ")).toEqual(["a.example.com", "b.example.com", "c.lan"]);
    expect(editSummary({ op: "addSite", domains: ["a.lan"], upstream: "app:80" })).toBe("add site a.lan → app:80");
    expect(editSummary({ op: "removeSite", site: "a.lan" })).toBe("remove site a.lan");
    expect(editSummary({ op: "replace", text: "x" })).toBe("edit the Caddyfile");
    expect(targetKey("caddy", "host")).toBe("caddy:host");
  });

  it("finds Caddy sites like the backend", () => {
    const s = sites("{\n\temail a@example.com\n}\n\na.lan {\n\treverse_proxy app:80\n}\n\nb.lan, c.lan {\n\treverse_proxy /api/* api:1\n\treverse_proxy web:2\n}\n");
    expect(s.map((x) => x.addresses)).toEqual([["a.lan"], ["b.lan", "c.lan"]]);
    expect(s[0].simple).toBe(true);
    expect(s[1].simple).toBe(false);
    expect(s[1].upstreams).toEqual(["api:1", "web:2"]);
  });

  it("diffs lines", () => {
    const d = diffLines("a\nb\nc\n", "a\nx\nc\nd\n");
    expect(d.filter((l) => l.tag !== "same").map((l) => `${l.tag}:${l.text}`).sort()).toEqual(["add:d", "add:x", "del:b"]);
    expect(d.filter((l) => l.tag === "add").length).toBe(2);
    expect(d.filter((l) => l.tag === "del").length).toBe(1);
  });

  it("previews and refuses stale saves in the mock", async () => {
    const t = await mockProxyEdit.targets(1);
    const c = t.caddy[0];
    const p = await mockProxyEdit.preview(c.text, { op: "setUpstream", site: "media.lan", upstream: "jellyfin:8096" });
    expect(p.text).toContain("reverse_proxy jellyfin:8096");
    await expect(mockProxyEdit.preview(c.text, { op: "setUpstream", site: "files.example.com", upstream: "x:1" })).rejects.toThrow(/edit it as text/);
    await expect(mockProxyEdit.saveCaddy(1, "host", p.text, "stale")).rejects.toThrow(/changed since/);
  });
});
