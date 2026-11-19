import { describe, expect, it } from "vitest";
import type { Service, ServiceStatus } from "../api";
import { certDays, emptyInput, healthOf, hostOfUrl, services, toInput, uptime } from "./services.svelte";

const NOW = 1_800_000_000;
const svc = (over: Partial<Service> = {}): Service => ({
  id: 1,
  name: "Media",
  url: "https://media.example.lan/",
  hostId: null,
  group: "Apps",
  description: "",
  icon: null,
  checkEnabled: true,
  expectStatus: "200-399",
  allowUntrusted: false,
  sortOrder: 1,
  source: "manual",
  ...over,
});
const status = (up: boolean, tls: ServiceStatus["result"]["tls"] = null): ServiceStatus => ({
  serviceId: 1,
  ts: NOW,
  result: { up, status: up ? 200 : 503, latencyMs: 12, error: up ? null : "HTTP 503", tls },
});
const tls = (trusted: boolean, days: number) => ({ trusted, problem: trusted ? null : "self-signed", expires: NOW + days * 86_400 + 60, subject: null, issuer: null, fingerprint: "ff" });

describe("services helpers", () => {
  it("derives health from checks and certificates", () => {
    expect(healthOf(svc({ checkEnabled: false }), status(true), NOW)).toBe("off");
    expect(healthOf(svc(), undefined, NOW)).toBe("pending");
    expect(healthOf(svc(), status(false), NOW)).toBe("down");
    expect(healthOf(svc(), status(true, tls(true, 60)), NOW)).toBe("up");
    expect(healthOf(svc(), status(true, tls(true, 5)), NOW)).toBe("warn");
    expect(healthOf(svc({ allowUntrusted: true }), status(true, tls(false, 60)), NOW)).toBe("warn");
    expect(healthOf(svc({ allowUntrusted: true }), status(true, tls(false, 60)), NOW, "ff")).toBe("up");
    expect(healthOf(svc({ allowUntrusted: true }), status(true, tls(false, 60)), NOW, "ee")).toBe("warn");
    expect(certDays(tls(true, 30), NOW)).toBe(30);
    expect(certDays(null, NOW)).toBeNull();
  });

  it("computes uptime and URL labels", () => {
    expect(uptime([])).toBeNull();
    expect(uptime([{ ts: 1, up: true, status: 200, latencyMs: 3 }, { ts: 2, up: false, status: null, latencyMs: null }])).toBe(50);
    expect(hostOfUrl("https://a.lan:8443/x")).toBe("a.lan:8443");
    expect(hostOfUrl("http://a.lan/")).toBe("a.lan");
    expect(hostOfUrl("not a url")).toBe("not a url");
  });

  it("round-trips a service into editor input", () => {
    const s = svc({ group: "Media", expectStatus: "200,401", allowUntrusted: true });
    expect(toInput(s)).toMatchObject({ name: "Media", group: "Media", expectStatus: "200,401", allowUntrusted: true });
    expect(emptyInput()).toMatchObject({ checkEnabled: true, expectStatus: "200-399" });
  });
});

describe("services store", () => {
  it("tracks status events, history and clears", () => {
    services.onEvent({ kind: "status", ...status(true), serviceId: 77 });
    services.onEvent({ kind: "status", ...status(false), serviceId: 77, ts: NOW + 60 });
    services.onEvent({ kind: "status", ...status(false), serviceId: 77, ts: NOW + 60 }); // duplicate ts ignored
    expect(services.status[77].result.up).toBe(false);
    expect(services.history[77]).toHaveLength(2);
    services.onEvent({ kind: "cleared", serviceId: 77 });
    expect(services.status[77]).toBeUndefined();
  });

  it("groups, filters and summarises the mock board", async () => {
    await services.refresh();
    expect(services.services.length).toBeGreaterThan(3);
    services.setGroupBy("group");
    const labels = services.groups.map((g) => g.label);
    expect(labels).toEqual(["Media", "Dev", "Network"]);
    services.filter = "git";
    expect(services.visible.map((s) => s.name)).toEqual(["Git"]);
    services.filter = "";
    services.setGroupBy("host");
    expect(services.groups.at(-1)?.label).toBe("no host");
    services.setGroupBy("group");

    const created = await services.save(0, { ...emptyInput(), name: "Wiki", url: "http://wiki.example.lan", group: "Docs" });
    expect(services.byId(created.id)?.group).toBe("Docs");
    await expect(services.save(0, { ...emptyInput(), name: "Bad", url: "nope" })).rejects.toBeTruthy();
    await services.remove(created.id);
    expect(services.byId(created.id)).toBeUndefined();
    expect(services.summary.total).toBe(services.services.length);
  });
});
