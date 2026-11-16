import { describe, expect, it } from "vitest";
import type { ContainerDetail, LogEventView } from "../api";
import { mockAlerts } from "../mockAlerts";
import { commandsFor, factsFrom, logTriage, stateLine, verdictOf } from "./logTriage";
import { systemPrompt } from "./prompt";

const CONVERTX = 'error: NOT_FOUND\n status: 404,\n   code: "NOT_FOUND"\n      at NotFoundError (/app/node_modules/elysia/src/error.ts:132:3)';
const STIRLING = "18:07:39.214 [jetty-1511] WARN  s.s.S.e.GlobalExceptionHandler - No resource at /api/v1/policies: No static resource api/v1/policies for request '/api/v1/policies'.";

const healthy = factsFrom("convertx", { state: "running", health: "healthy", startedAt: "2026-11-16T15:00:00Z", restartCount: 0, exitCode: 0 } as ContainerDetail, null);
const now = Math.floor(Date.parse("2026-11-16T18:12:00Z") / 1000);

describe("log triage", () => {
  it("calls a one-off 404 on a healthy container likely harmless", () => {
    const events = mockAlerts.classify(CONVERTX);
    expect(events).toHaveLength(1);
    const t = logTriage({ title: "convertx on Lumen", source: "container/convertx", events, container: healthy, now });
    expect(t.verdict).toBe("harmless");
    const text = t.check.items.map((i) => i.text).join("\n");
    expect(text).toContain("Container state now: running · healthy · up 3 h 12 min · restarted 0 times");
    expect(text).toContain("likely harmless: an HTTP 404");
    expect(text).toContain("no action needed. The container is running and healthy");
    expect(text).not.toMatch(/path\/to|INPUT_FILE|Usual cause/);
    expect(t.question).toContain("likely harmless");
  });

  it("calls a WARN line harmless", () => {
    const t = logTriage({ title: "stirling-pdf", source: "container/stirling-pdf", events: mockAlerts.classify(STIRLING), container: null, now });
    expect(t.verdict).toBe("harmless");
  });

  it("keeps real failures as problems", () => {
    const err: LogEventView[] = [{ head: "ERROR db down", lines: 1, level: "error", notFound: false, harmless: null }];
    expect(verdictOf(err, { ...healthy, state: "restarting" })).toBe("problem");
    expect(verdictOf(err, { ...healthy, health: "unhealthy" })).toBe("problem");
    expect(verdictOf([{ ...err[0], level: "fatal" }], healthy)).toBe("problem");
    expect(verdictOf(err, healthy)).toBe("unclear");
    expect(verdictOf([err[0], err[0], err[0]], healthy)).toBe("problem");
  });

  it("says unknown state plainly and offers only runnable commands", () => {
    expect(stateLine(factsFrom("x", null, "Docker isn't reachable"), now)).toBe("unknown (couldn't read it: Docker isn't reachable)");
    const cmds = commandsFor("container/convertx");
    expect(cmds[0]).toContain("docker inspect -f");
    expect(cmds[0]).toContain("'convertx'");
    expect(cmds.join(" ")).not.toMatch(/<|path\/to/);
    expect(commandsFor("journal/sshd")[0]).toContain("journalctl -t 'sshd'");
  });

  it("tells the model not to guess", () => {
    const p = systemPrompt();
    expect(p).toContain("Never invent file paths, environment variables");
    expect(p).toContain("don't call it failed");
  });
});
