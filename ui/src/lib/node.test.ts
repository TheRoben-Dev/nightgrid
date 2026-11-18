import { describe, expect, it } from "vitest";
import type { NodeView } from "./api";
import { capsText, installAction, newer, roleOf, spanText, stateTone, syncText, unsupportedArch } from "./node";
import type { NodeSetup } from "./api";

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
    // A NAS on 32-bit ARM has no build: no Install button.
    expect(installAction(view({ state: "notInstalled", version: null, arch: "armv7l" }), rel)).toBeNull();
    expect(unsupportedArch(view({ arch: "armv7l" }))).toBe("armv7l");
    expect(unsupportedArch(view({ arch: "aarch64" }))).toBeNull();
    expect(unsupportedArch(view({ arch: "" }))).toBeNull();
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

  it("tells hub, members and direct hosts apart", () => {
    const hub = { hostId: 1, hubId: "h1" };
    const setup = (hubId: string | null) => ({ link: { hubId } }) as unknown as NodeSetup;
    expect(roleOf(view({ hostId: 1 }), setup("h1"), hub)).toBe("hub");
    expect(roleOf(view({ hostId: 2 }), setup("h1"), hub)).toBe("member");
    expect(roleOf(view({ hostId: 3 }), setup("other"), hub)).toBe("otherHub");
    expect(roleOf(view({ hostId: 4 }), setup(null), hub)).toBe("direct");
    expect(roleOf(view({ hostId: 5, state: "notInstalled" }), undefined, null)).toBe("none");
  });
});
