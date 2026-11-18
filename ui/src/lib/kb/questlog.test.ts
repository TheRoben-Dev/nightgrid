import { describe, expect, it } from "vitest";
import { ARTICLES, fromFile, parts, questable, stepChecks, stepList } from "./library";
import { questEvents, questProgress } from "./quest";

const GUIDE = fromFile(
  "/docs/kb/guides/demo.md",
  `# Demo

Intro.

## Prepare

- [ ] Add the host {check: ssh-key}
- [ ] Install Docker {check: docker}

## Run it

Some text.

- [ ] Start the stack
- [ ] Open the page
`,
)!;

describe("quest log", () => {
  it("lists steps with their part, text and check, keyed like parts()", () => {
    const steps = stepList(GUIDE);
    expect(steps.map((s) => s.text)).toEqual(["Add the host", "Install Docker", "Start the stack", "Open the page"]);
    expect(steps.map((s) => s.part)).toEqual([0, 0, 1, 1]);
    expect(steps[2].partTitle).toBe("Run it");
    expect(steps.map((s) => s.check)).toEqual(["ssh-key", "docker", null, null]);
    expect(steps.map((s) => s.key)).toEqual(parts(GUIDE).flatMap((p) => p.keys));
    const checks = stepChecks(GUIDE);
    for (const s of steps) expect(checks[s.key] ?? null).toBe(s.check);
  });

  it("matches parts() and stepChecks() on every bundled guide", () => {
    for (const a of ARTICLES.filter(questable)) {
      const steps = stepList(a);
      expect(steps.map((s) => s.key), a.id).toEqual(parts(a).flatMap((p) => p.keys));
      expect(new Set(steps.map((s) => s.part)).size, a.id).toBe(parts(a).length);
      const checks = stepChecks(a);
      for (const s of steps) expect(checks[s.key] ?? null, a.id).toBe(s.check);
    }
  });

  it("marks guides with steps as quests, but not the Nightgrid or basics shelves", () => {
    expect(questable(GUIDE)).toBe(true);
    expect(ARTICLES.filter(questable).length).toBeGreaterThan(20);
    expect(ARTICLES.some((a) => a.shelf === "nightgrid" && questable(a))).toBe(false);
    expect(ARTICLES.some((a) => a.shelf === "basics" && questable(a))).toBe(false);
  });

  it("finds the current step and objective", () => {
    const steps = stepList(GUIDE);
    const [a, b, c] = steps.map((s) => s.key);
    expect(questProgress(steps, []).current?.text).toBe("Add the host");
    const p = questProgress(steps, [a, b]);
    expect(p.current?.text).toBe("Start the stack");
    expect(p.objectives).toEqual({ done: 1, total: 2 });
    // Out of order: the first step not done is current.
    expect(questProgress(steps, [b, c]).current?.text).toBe("Add the host");
    expect(questProgress(steps, steps.map((s) => s.key)).complete).toBe(true);
  });

  it("says what happened, biggest news first", () => {
    const steps = stepList(GUIDE);
    const k = steps.map((s) => s.key);
    expect(questEvents(steps, [], [k[0]])).toEqual([{ kind: "updated", label: "QUEST UPDATED", title: "Add the host", next: "Install Docker" }]);
    expect(questEvents(steps, [k[0]], [k[0], k[1]])[0]).toMatchObject({ kind: "objective", label: "OBJECTIVE COMPLETE", title: "Prepare", next: "Start the stack" });
    expect(questEvents(steps, k.slice(0, 3), k)[0]).toMatchObject({ kind: "complete", label: "QUEST COMPLETE" });
    expect(questEvents(steps, [k[0]], [k[2], k[0]])[0]).toMatchObject({ kind: "updated", title: "Start the stack", next: "Install Docker" });
    expect(questEvents(steps, [k[0], k[1]], [k[0]])).toEqual([]);
    expect(questEvents(steps, [], [k[2], k[3]])[0]).toMatchObject({ kind: "objective", title: "Run it" });
  });
});
