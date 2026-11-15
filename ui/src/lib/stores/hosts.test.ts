import { describe, expect, it } from "vitest";
import type { Group, Host } from "../api";
import { groupHosts, hostMatches } from "./hosts.svelte";

const host = (id: number, name: string, extra: Partial<Host> = {}): Host => ({
  id,
  name,
  hostname: `${name}.lan`,
  port: 22,
  username: null,
  groupId: null,
  color: null,
  icon: null,
  favorite: false,
  notes: null,
  authMethod: "agent",
  identityFile: null,
  proxyJump: null,
  rememberSecret: false,
  sshAlias: null,
  tailnetAddress: null,
  driftHistory: false,
  tags: [],
  ...extra,
});

describe("host search", () => {
  const h = host(1, "lumen", { tags: ["docker", "prod"], username: "admin" });
  it("matches fuzzy on name, hostname, user and tags", () => {
    expect(hostMatches(h, "lmn")).toBe(true);
    expect(hostMatches(h, "admin")).toBe(true);
    expect(hostMatches(h, "xyz")).toBe(false);
    expect(hostMatches(h, "")).toBe(true);
  });
  it("supports #tag prefix filters", () => {
    expect(hostMatches(h, "#dock")).toBe(true);
    expect(hostMatches(h, "#lumen")).toBe(false);
  });
});

describe("groupHosts", () => {
  const groups: Group[] = [
    { id: 10, name: "Servers", color: "#00E5FF", icon: null, sortOrder: 1 },
    { id: 11, name: "Edge", color: null, icon: null, sortOrder: 0 },
  ];
  const hs = [
    host(1, "lumen", { groupId: 10, favorite: true }),
    host(2, "nas", { groupId: 10 }),
    host(3, "router", { groupId: 11 }),
    host(4, "laptop"),
  ];

  it("orders favorites, groups by sort order, then ungrouped", () => {
    const g = groupHosts(hs, groups, "");
    expect(g.map((x) => x.name)).toEqual(["Favorites", "Edge", "Servers", "Ungrouped"]);
    expect(g[2].hosts.map((h) => h.name)).toEqual(["nas"]);
  });

  it("hides empty groups when filtering", () => {
    expect(groupHosts(hs, groups, "router").map((x) => x.name)).toEqual(["Edge"]);
  });

  it("labels a flat list as all hosts", () => {
    expect(groupHosts([host(1, "a")], [], "")[0].name).toBe("All hosts");
  });
});
