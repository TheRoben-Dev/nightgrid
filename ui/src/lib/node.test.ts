import { describe, expect, it } from "vitest";
import type { NodeView } from "./api";
import { capsText, installAction, newer, spanText, stateTone, syncText } from "./node";

const view = (over: Partial<NodeView>): NodeView =>
  ({
    hostId: 1,
    state: "ready",
    message: null,
    useNode: true,
    version: "0.23.0",
    nodeId: "ab",
    proto: 2,
    caps: [],
    eventsLive: true,
    metricsLive: false,
    syncing: false,
    syncedAt: null,
    lastSync: null,
    rulesNote: null,
    user: "alice",
    arch: "x86_64",
    info: null,
    ...over,
  }) as NodeView;

describe("node", () => {
  it("compares versions by number", () => {
    expect(newer("v0.23.1", "0.23.0")).toBe(true);
    expect(newer("0.10.0", "0.9.9")).toBe(true);
    expect(newer("v0.23.0", "0.23.0")).toBe(false);
    expect(newer("0.23", "0.23.0")).toBe(false);
  });

  it("offers the right action", () => {
    const rel = { tag: "v0.24.0", published: "", assets: [] };
    expect(installAction(view({ state: "notInstalled", version: null }), rel)).toBe("Install");
    expect(installAction(view({ state: "noAccess" }), rel)).toBe("Fix access");
    expect(installAction(view({}), rel)).toBe("Update");
    expect(installAction(view({ version: "0.24.0" }), rel)).toBeNull();
    expect(installAction(view({}), null)).toBeNull();
    expect(installAction(view({ state: "off" }), rel)).toBeNull();
  });

  it("describes syncs, spans and caps", () => {
    expect(syncText(null)).toBe("not synced yet");
    expect(syncText({ points: 1440, diskRows: 0, hours: 1, events: 2, reset: false, pruned: false })).toBe("1,440 points · 2 events · 1 uptime hour");
    expect(syncText({ points: 0, diskRows: 0, hours: 0, events: 0, reset: true, pruned: false })).toBe("nothing new (the node's history started over)");
    expect(spanText(14 * 86400)).toBe("14 d");
    expect(spanText(5 * 3600)).toBe("5 h");
    expect(capsText(["metrics", "events", "net", "logs"])).toBe("metrics, events +2 more");
    expect(stateTone("noAccess")).toBe("warn");
  });
});
