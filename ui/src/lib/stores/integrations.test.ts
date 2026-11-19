import { describe, expect, it } from "vitest";
import type { IntegrationStatus, IntegrationView } from "../api";
import { emptyIntegrationInput, KIND_GROUPS, KIND_META, kindsByGroup, portainerUrl, summary, toneOf } from "./integrations.svelte";

const view = (over: Partial<IntegrationView["integration"]>): IntegrationView => ({
  hasSecret: false,
  integration: {
    id: 1,
    kind: "portainer",
    name: "Portainer",
    baseUrl: "https://portainer.lan:9443",
    username: "",
    slug: "",
    endpointId: 2,
    hostId: 7,
    allowUntrusted: false,
    ...over,
  },
});

describe("integrations helpers", () => {
  it("summarises statuses", () => {
    const plex: IntegrationStatus = {
      kind: "plex",
      server: "media01",
      version: "1",
      transcodes: 1,
      streams: [
        { user: "a", title: "t", media: "movie", player: "p", state: "playing", progress: 10, transcode: true, quality: null },
        { user: "b", title: "t", media: "movie", player: "p", state: "paused", progress: null, transcode: false, quality: null },
      ],
    };
    expect(summary(plex)).toBe("2 streams · 1 transcoding");
    expect(summary({ ...plex, streams: [], transcodes: 0 })).toBe("Nothing playing");
    expect(
      summary({
        kind: "amp",
        instances: [
          { name: "a", friendlyName: "A", module: "M", running: true, state: "ready", users: 3, maxUsers: 10, cpuPercent: null, memoryMb: null },
          { name: "b", friendlyName: "B", module: "M", running: false, state: "stopped", users: null, maxUsers: null, cpuPercent: null, memoryMb: null },
        ],
      }),
    ).toBe("1/2 running · 3 players");
    expect(summary({ kind: "kuma", title: "", up: 3, down: 1, monitors: [{}, {}, {}, {}] as never })).toBe("3/4 up · 1 down");
    expect(summary({ kind: "portainer", version: "2.21.0" })).toBe("v2.21.0");
    expect(summary(null)).toBe("");
  });

  it("tones", () => {
    expect(toneOf(undefined)).toBe("idle");
    expect(toneOf({ status: null, error: "401", loading: false, at: 1 })).toBe("bad");
    expect(toneOf({ status: { kind: "kuma", title: "", up: 1, down: 1, monitors: [] }, error: null, loading: false, at: 1 })).toBe("warn");
    expect(toneOf({ status: { kind: "portainer", version: null }, error: null, loading: false, at: 1 })).toBe("ok");
  });

  it("builds Portainer links only for the linked host", () => {
    const list = [view({}), view({ id: 2, kind: "plex", hostId: 3 })];
    expect(portainerUrl(list, 7, "abc")).toBe("https://portainer.lan:9443/#!/2/docker/containers/abc");
    expect(portainerUrl(list, 3, "abc")).toBeNull();
    expect(portainerUrl(list, null, "abc")).toBeNull();
  });

  it("defaults the Portainer environment to 1", () => {
    expect(emptyIntegrationInput("portainer").endpointId).toBe(1);
    expect(emptyIntegrationInput("plex").endpointId).toBe(0);
  });

  it("summarises and tones app statuses", () => {
    const app: IntegrationStatus = {
      kind: "app",
      version: "4.0",
      summary: "3 in queue · 12 missing",
      health: "warn",
      tiles: [],
      rowsTitle: "Queue",
      rows: [],
      more: 0,
      notes: ["Indexers unavailable"],
      actions: [],
    };
    expect(summary(app)).toBe("3 in queue · 12 missing");
    expect(toneOf({ status: app, error: null, loading: false, at: 1 })).toBe("warn");
    expect(toneOf({ status: { ...app, health: "bad" }, error: null, loading: false, at: 1 })).toBe("bad");
    expect(toneOf({ status: { ...app, health: "ok" }, error: null, loading: false, at: 1 })).toBe("ok");
  });

  it("describes every kind and groups them for the picker", () => {
    for (const [k, m] of Object.entries(KIND_META)) {
      expect(m.name, k).not.toBe("");
      expect(m.placeholder, k).toMatch(/^https?:\/\//);
      expect(KIND_GROUPS, k).toContain(m.group);
    }
    const all = kindsByGroup().flatMap((g) => g.kinds);
    expect(all.length).toBe(Object.keys(KIND_META).length);
    expect(kindsByGroup("sonarr").flatMap((g) => g.kinds)).toEqual(["sonarr"]);
    expect(kindsByGroup("media automation").every((g) => g.group === "Media automation")).toBe(true);
    expect(kindsByGroup("zzz-nothing")).toEqual([]);
  });
});
