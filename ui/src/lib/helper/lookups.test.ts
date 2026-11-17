import { describe, expect, it } from "vitest";
import { CHECKS, followUpHint, lookupBlock, notYetRun, pathsIn, planLookups, takeCheckRequests, wantsLookups } from "./lookups";
import { contextBlock, estimateTokens, fitTurns } from "./prompt";

const kinds = (q: string) => planLookups(q).map((l) => (l.target ? `${l.kind}:${l.target}` : l.kind));

describe("DRIFT read-only checks", () => {
  it("picks checks from the question's words", () => {
    expect(kinds("why is the disk full on server01")).toEqual(expect.arrayContaining(["disk", "storage"]));
    expect(kinds("server01 feels slow")).toContain("processes");
    expect(kinds("which containers keep restarting on server01")).toEqual(expect.arrayContaining(["containers", "dockerEvents"]));
    expect(kinds("is the nfs share mounted on server01")).toContain("mounts");
    expect(kinds("any pending updates on server01?")).toContain("updates");
    expect(kinds("who logged in to server01 today")).toContain("logins");
  });

  it("takes a broad look when nothing specific is asked", () => {
    expect(kinds("what's going on with server01?")).toEqual(["overview", "failedUnits", "errors", "containers", "dockerEvents", "kernelLog"]);
  });

  it("reads files the question names", () => {
    expect(pathsIn("check /etc/nginx/nginx.conf and ~/stack/compose.yaml please")).toEqual(["/etc/nginx/nginx.conf", "~/stack/compose.yaml"]);
    expect(kinds("what's in /etc/fstab on server01")).toContain("file:/etc/fstab");
    expect(pathsIn("and/or 1/2")).toEqual([]);
  });

  it("skips how-to questions about Nightgrid", () => {
    expect(wantsLookups("how do I add a tunnel to server01?")).toBe(false);
    expect(wantsLookups("where is the docker view for server01")).toBe(false);
    expect(wantsLookups("how do I find out why nginx is down on server01")).toBe(true);
    expect(wantsLookups("is server01 ok?")).toBe(true);
  });

  it("caps a plan", () => {
    expect(planLookups("disk memory cpu temp ports network docker logs timers mounts updates logins").length).toBeLessThanOrEqual(8);
  });

  it("parses the model's check requests and drops them from the text", () => {
    const r = takeCheckRequests("Looks like the worker can't reach the db.\n[[check container-logs worker]]\n[[check disk]]\n[[check container]]\n[[check rm -rf]]\n[[check bogus]]");
    expect(r.text).toBe("Looks like the worker can't reach the db.");
    expect(r.asked).toEqual([
      { kind: "containerLogs", target: "worker" },
      { kind: "disk", target: null },
    ]);
    expect(takeCheckRequests("[[check a]]\n[[check disk]]\n[[check memory]]\n[[check ports]]\n[[check network]]").asked).toHaveLength(3);
  });

  it("doesn't run a check twice", () => {
    const done = [{ lookup: { kind: "disk" as const, target: null }, command: "df -hP", ok: true, output: "x", clipped: false }];
    expect(notYetRun([{ kind: "disk", target: null }, { kind: "memory", target: null }], done)).toEqual([{ kind: "memory", target: null }]);
  });

  it("builds the prompt block, with secrets masked again", () => {
    const block = lookupBlock("server01", [
      { lookup: { kind: "containerLogs", target: "web" }, command: "docker logs --tail 50 'web' 2>&1", ok: true, output: "boot\nAPI_TOKEN=abc123456789", clipped: true },
      { lookup: { kind: "service", target: "nginx.service" }, command: "", ok: false, output: "not allowed", clipped: false },
    ]);
    expect(block).toContain("### server01\n#### container-logs web (`docker logs --tail 50 'web' 2>&1`) [cut to fit]");
    expect(block).toContain("Couldn't check: not allowed");
    const ctx = contextBlock({ view: "overview", lookups: [block], lookupHint: followUpHint() }, []);
    expect(ctx).toContain("SERVER CHECKS");
    expect(ctx).not.toContain("abc123456789");
    expect(ctx).toContain("[[check KIND]]");
  });

  it("shares a host's space between its checks and fits the model's window", () => {
    const big = (kind: "disk" | "containerLogs") => ({ lookup: { kind, target: kind === "disk" ? null : "web" }, command: "x", ok: true, output: Array.from({ length: 600 }, (_, i) => `line ${i}`).join("\n"), clipped: false });
    const block = lookupBlock("server01", [big("disk"), big("containerLogs")]);
    expect(block).toContain("line 0\n");
    expect(block).toContain("line 599");
    expect(block).toContain("[older lines cut]");
    expect(block.length).toBeLessThan(6000);
    const blocks = [block, lookupBlock("server02", [big("disk")])];
    const roomy = fitTurns([], "why is server01 slow", { view: "overview", lookups: blocks, lookupHint: followUpHint() }, [], 16384);
    expect(roomy.trimmed).toEqual([]);
    const sys = estimateTokens(roomy.turns[0].content);
    // A window with room for the system prompt plus about 1,000 tokens: the checks shrink to fit.
    const small = fitTurns([], "why is server01 slow", { view: "overview", lookups: blocks, lookupHint: followUpHint() }, [], sys + 1100 + 768 + 64);
    expect(small.trimmed).toContain("part of the live facts");
    expect(small.tokens).toBeLessThanOrEqual(sys + 1100);
    const fit = small;
    expect(fit.turns[fit.turns.length - 1].content).toContain("SERVER CHECKS");
  });

  it("names every check once", () => {
    expect(new Set(CHECKS.map((c) => c.name)).size).toBe(CHECKS.length);
    expect(new Set(CHECKS.map((c) => c.kind)).size).toBe(CHECKS.length);
  });
});
