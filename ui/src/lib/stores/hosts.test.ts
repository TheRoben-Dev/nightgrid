import { describe, expect, it } from "vitest";
import type { Group, Host } from "../api";
import { groupHosts, hostMatches, hosts } from "./hosts.svelte";

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
  macAddress: null,
  wakeRelayId: null,
  sleepWindow: null,
  role: "server",
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

describe("default host per view", () => {
  it("never starts on a client, and remembers picks per view", () => {
    localStorage.clear();
    hosts.hosts = [host(1, "ally", { role: "client", favorite: true }), host(2, "lumen"), host(3, "nas")];
    expect(hosts.preferred("docker")?.name).toBe("lumen");
    hosts.remember("docker", 3);
    expect(hosts.preferred("docker")?.name).toBe("nas");
    // Another view follows the last pick anywhere until it has its own.
    expect(hosts.preferred("system")?.name).toBe("nas");
    hosts.remember("system", 1); // chosen by hand: a client is fine then
    expect(hosts.preferred("system")?.name).toBe("ally");
    expect(hosts.preferred("docker")?.name).toBe("nas");
    hosts.hosts = [host(1, "ally", { role: "client" })];
    localStorage.clear();
    expect(hosts.preferred("files")?.name).toBe("ally"); // only clients: still something
  });
});
