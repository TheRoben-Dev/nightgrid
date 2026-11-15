import { describe, expect, it } from "vitest";
import type { Group, Host } from "../api";
import { mockAutomation, mockVariables } from "../mockAutomation";
import { applyEvent, confirmWord, fmtMs, MAX_LINES, newHostRun, scopeApplies, type RunState } from "./automation.svelte";

const host = (id: number, groupId: number | null): Host => ({ id, name: `server0${id}`, groupId }) as unknown as Host;
const group = (id: number, name: string): Group => ({ id, name }) as unknown as Group;

describe("automation helpers", () => {
  it("asks for the group name only when a whole group is picked", () => {
    const hosts = [host(1, 10), host(2, 10), host(3, null)];
    const groups = [group(10, "web")];
    expect(confirmWord([1], hosts, groups, null)).toBeNull();
    expect(confirmWord([1, 2], hosts, groups, 10)).toBe("web");
    // Picked a group, then added a host: fall back to the count.
    expect(confirmWord([1, 2, 3], hosts, groups, 10)).toBe("3");
    expect(confirmWord([1, 3], hosts, groups, null)).toBe("2");
  });

  it("matches snippet scopes", () => {
    const h = host(1, 10);
    expect(scopeApplies({ kind: "global" }, h)).toBe(true);
    expect(scopeApplies({ kind: "host", hostId: 1 }, h)).toBe(true);
    expect(scopeApplies({ kind: "host", hostId: 2 }, h)).toBe(false);
    expect(scopeApplies({ kind: "group", groupId: 10 }, h)).toBe(true);
    expect(scopeApplies({ kind: "group", groupId: 11 }, h)).toBe(false);
  });

  it("applies run events", () => {
    const run: RunState = { id: 1, title: "x", hostIds: [1, 2], stepLabels: [], hosts: { 1: newHostRun(1), 2: newHostRun(2) }, done: false, stopped: false, startedAt: 0 };
    applyEvent(run, { kind: "hostStart", hostId: 1 });
    applyEvent(run, { kind: "stepStart", hostId: 1, step: 0, label: "$ uptime" });
    applyEvent(run, { kind: "lines", hostId: 1, lines: [{ stream: "out", text: "up 3 days" }] });
    applyEvent(run, { kind: "stepEnd", hostId: 1, step: 0, code: 0, error: null, ms: 12 });
    applyEvent(run, { kind: "hostEnd", hostId: 1, ok: true, code: 0, error: null, needs: null, ms: 40, stopped: false });
    expect(run.hosts[1]).toMatchObject({ status: "ok", code: 0, ms: 40 });
    expect(run.hosts[1].steps[0]).toMatchObject({ status: "ok", ms: 12 });
    applyEvent(run, { kind: "hostEnd", hostId: 2, ok: false, code: null, error: "needs a login", needs: "login", ms: 5, stopped: false });
    expect(run.hosts[2]).toMatchObject({ status: "failed", needs: "login" });
    applyEvent(run, { kind: "done", stopped: false });
    expect(run.done).toBe(true);

    // Output is capped per host.
    const big = Array.from({ length: MAX_LINES + 10 }, (_, i) => ({ stream: "out" as const, text: String(i) }));
    applyEvent(run, { kind: "lines", hostId: 1, lines: big });
    expect(run.hosts[1].lines.length).toBe(MAX_LINES);
    expect(run.hosts[1].dropped).toBe(11);
  });

  it("marks a step stopped without code or error", () => {
    const run: RunState = { id: 1, title: "x", hostIds: [1], stepLabels: [], hosts: { 1: newHostRun(1) }, done: false, stopped: false, startedAt: 0 };
    applyEvent(run, { kind: "stepEnd", hostId: 1, step: 0, code: null, error: null, ms: 1 });
    expect(run.hosts[1].steps[0].status).toBe("stopped");
    applyEvent(run, { kind: "stepEnd", hostId: 1, step: 1, code: 2, error: null, ms: 1 });
    expect(run.hosts[1].steps[1].status).toBe("failed");
  });

  it("formats durations", () => {
    expect(fmtMs(250)).toBe("250 ms");
    expect(fmtMs(2500)).toBe("2.5 s");
    expect(fmtMs(125_000)).toBe("2m 5s");
  });
});

describe("browser mock", () => {
  it("finds variables like the backend", () => {
    expect(mockVariables(["docker logs --tail {{lines=100}} {{container}} {{host}}"])).toEqual([
      { name: "lines", default: "100" },
      { name: "container", default: null },
    ]);
  });

  it("inspects danger and missing values", async () => {
    const r = await mockAutomation.inspect({ kind: "command", command: "docker system prune -f; rm {{path}}", sudo: false }, {}, "server01");
    expect(r.destructive).toEqual(expect.arrayContaining(["docker prune", "rm"]));
    expect(r.missing).toEqual(["path"]);
    const ok = await mockAutomation.inspect({ kind: "command", command: "cat {{path}}", sudo: false }, { path: "a b" }, "server01");
    expect(ok.preview).toBe("cat 'a b'");
    expect(ok.destructive).toEqual([]);
  });
});
