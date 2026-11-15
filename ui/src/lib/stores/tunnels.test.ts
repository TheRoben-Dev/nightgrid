import { describe, expect, it } from "vitest";
import type { TunnelView } from "../api";
import { emptyTunnel, isActive, localEndpoint, toTunnelInput, tunnels, tunnelTarget } from "./tunnels.svelte";

const view = (over: Partial<TunnelView> = {}): TunnelView => ({
  id: 1,
  hostId: 1,
  name: "",
  kind: "local",
  bindAddr: "127.0.0.1",
  bindPort: 0,
  destHost: "localhost",
  destPort: 80,
  autoStart: false,
  temporary: false,
  spec: "-L 127.0.0.1:0:localhost:80",
  public: false,
  state: { kind: "stopped" },
  ...over,
});

describe("tunnel helpers", () => {
  it("rewrites URLs to the local end of a tunnel", () => {
    const t = tunnelTarget("https://app.example.lan/admin?x=1#top")!;
    expect([t.host, t.port]).toEqual(["app.example.lan", 443]);
    expect(t.rebuild(50123)).toBe("https://127.0.0.1:50123/admin?x=1#top");
    expect(tunnelTarget("http://[fd00::5]:8080/")!.host).toBe("fd00::5");
    expect(tunnelTarget("http://server01")!.port).toBe(80);
    expect(tunnelTarget("ftp://x")).toBeNull();
  });

  it("shows where a running tunnel listens", () => {
    expect(localEndpoint(view(), { kind: "running", boundPort: 50001 })).toBe("127.0.0.1:50001");
    expect(localEndpoint(view({ bindAddr: "0.0.0.0" }), { kind: "running", boundPort: 9 })).toBe("127.0.0.1:9");
    expect(localEndpoint(view({ bindAddr: "::1" }), { kind: "running", boundPort: 9 })).toBe("[::1]:9");
    expect(localEndpoint(view({ kind: "remote" }), { kind: "running", boundPort: 9 })).toBeNull();
    expect(localEndpoint(view(), { kind: "starting" })).toBeNull();
    expect(isActive({ kind: "retrying", message: "x", retrySecs: 5 })).toBe(true);
    expect(isActive({ kind: "failed", message: "x", needsLogin: false })).toBe(false);
  });

  it("builds editor input", () => {
    expect(emptyTunnel(3, "dynamic")).toMatchObject({ hostId: 3, kind: "dynamic", bindPort: 1080 });
    expect(toTunnelInput(view({ name: "web", autoStart: true }))).toMatchObject({ name: "web", autoStart: true, destPort: 80 });
  });
});

describe("tunnels store", () => {
  it("tracks states and turns counters into rates", () => {
    tunnels.onEvent({ kind: "state", id: 501, state: { kind: "running", boundPort: 8080 } });
    expect(tunnels.activeCount).toBeGreaterThanOrEqual(1);
    tunnels.onEvent({ kind: "stats", stats: [{ id: 501, stats: { rx: 100, tx: 50, active: 1, total: 1 } }] });
    tunnels.onEvent({ kind: "stats", stats: [{ id: 501, stats: { rx: 1100, tx: 150, active: 1, total: 2 } }] });
    expect(tunnels.rates[501]).toEqual([0, 1100]);
    expect(tunnels.traffic[501].total).toBe(2);
    tunnels.onEvent({ kind: "state", id: 501, state: { kind: "stopped" } });
    expect(tunnels.rates[501]).toBeUndefined();
  });

  it("creates, toggles and deletes against the mock", async () => {
    await tunnels.refresh();
    const before = tunnels.tunnels.length;
    await tunnels.save(0, { ...emptyTunnel(1), name: "test", destPort: 3000 }, false);
    expect(tunnels.tunnels.length).toBe(before + 1);
    const t = tunnels.tunnels.find((x) => x.name === "test")!;
    expect(t.spec).toBe("-L 127.0.0.1:8080:localhost:3000");
    await tunnels.remove(t.id);
    expect(tunnels.byId(t.id)).toBeUndefined();
    await expect(tunnels.save(0, { ...emptyTunnel(1), destPort: 0 }, false)).rejects.toBeTruthy();
  });
});
