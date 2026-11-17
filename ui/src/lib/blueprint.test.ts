import { describe, expect, it } from "vitest";
import type { BpStep } from "./api";
import { groups, initialPick, passphraseProblem, resultsLine, toggleStep } from "./blueprint";

const s = (id: string, kind: BpStep["kind"], state: BpStep["state"]): BpStep => ({ id, kind, title: id, detail: "", state, defaultOn: state === "new" });
const steps = [s("stack:media", "stackFiles", "new"), s("start:media", "stackStart", "new"), s("cron:", "cron", "same"), s("fw:deny 23", "firewall", "manual"), s("caddy:host", "caddy", "differs")];

describe("blueprints", () => {
  it("groups steps and picks the new ones", () => {
    expect(groups(steps).map((g) => g.label)).toEqual(["Stacks", "Cron", "Firewall", "Proxy"]);
    expect([...initialPick(steps)]).toEqual(["stack:media", "start:media"]);
  });

  it("keeps starting a stack tied to writing it", () => {
    let p = toggleStep(new Set(), steps, "start:media");
    expect([...p].sort()).toEqual(["stack:media", "start:media"]);
    p = toggleStep(p, steps, "stack:media");
    expect([...p]).toEqual([]);
  });

  it("checks passphrases and words results", () => {
    expect(passphraseProblem("short", "short")).toMatch(/at least/);
    expect(passphraseProblem("long enough", "long enougH")).toMatch(/match/);
    expect(passphraseProblem("long enough", "long enough")).toBeNull();
    expect(resultsLine([{ id: "a", ok: true, message: "" }])).toBe("The step was applied.");
    expect(resultsLine([{ id: "a", ok: true, message: "" }, { id: "b", ok: false, message: "" }])).toBe("1 of 2 steps applied, 1 failed.");
  });
});
