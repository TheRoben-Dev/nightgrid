import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SECTIONS } from "../sections";
import { tour } from "../stores/tour.svelte";
import { ui } from "../stores/ui.svelte";
import { parseReply, systemPrompt } from "./prompt";
import { thingsHere, tourById, TOURS, toursFor, unknownTargets } from "./tours";

describe("tours", () => {
  it("every target is a known guide target", () => {
    expect(unknownTargets()).toEqual([]);
  });
  it("ids are unique and views are real", () => {
    expect(new Set(TOURS.map((t) => t.id)).size).toBe(TOURS.length);
    const views = SECTIONS.map((s) => s.id as string);
    for (const t of TOURS) {
      expect(t.steps.length, t.id).toBeGreaterThan(0);
      for (const v of t.views) expect(views, t.id).toContain(v);
      for (const s of t.steps) if (s.wait.kind === "view") expect(views, t.id).toContain(s.wait.view);
      // A click step needs something to click.
      for (const s of t.steps) if (s.wait.kind === "click") expect(s.target, t.id).toBeTruthy();
    }
  });
  it("lists the tours that fit a view first", () => {
    const list = toursFor("fleet");
    expect(list[0].id).toBe("fleet-health");
    expect(list).toHaveLength(TOURS.length);
  });
  it("what can I do here keeps the view's controls that are on screen", () => {
    const got = thingsHere("overview", (id) => id === "dashboard-edit");
    expect(got.map((t) => t.id)).toEqual(["dashboard-edit"]);
  });
  it("DRIFT can offer a known tour, once", () => {
    const r = parseReply("Sure.\n[[tour fleet-health]]\n[[tour add-host]]\n[[tour nope]]");
    expect(r.text).toBe("Sure.");
    expect(r.actions).toEqual([{ kind: "tour", id: "fleet-health" }]);
    expect(parseReply("[[tour nope]]").actions).toEqual([]);
    expect(systemPrompt()).toContain("[[tour TOUR]]");
    expect(systemPrompt()).toContain("first-run");
  });
});

describe("tour store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    document.body.innerHTML = "";
  });
  afterEach(() => {
    tour.stop();
    vi.useRealTimers();
  });

  it("next steps go on with Next and the last one finishes", () => {
    expect(tour.start("back-up-setup")).toBe(true);
    expect(tour.index).toBe(0);
    ui.go("settings");
    vi.advanceTimersByTime(250);
    expect(tour.index).toBe(1); // the view step saw Settings open
    tour.next();
    expect(tour.index).toBe(2);
    tour.back();
    expect(tour.index).toBe(1);
    tour.next();
    tour.next();
    expect(tour.tour).toBeNull();
  });

  it("waits for a dialog to appear and to go away, and never acts itself", () => {
    tour.start("add-host");
    vi.advanceTimersByTime(600);
    expect(tour.index).toBe(0);
    expect(tour.missing).toBe(true); // no + button on this bare page
    const d = document.createElement("dialog");
    d.setAttribute("open", "");
    document.body.append(d);
    vi.advanceTimersByTime(250);
    expect(tour.index).toBe(1);
    vi.advanceTimersByTime(600);
    expect(tour.index).toBe(1);
    d.remove();
    vi.advanceTimersByTime(250);
    expect(tour.index).toBe(2);
  });

  it("a click step moves on only when the user clicks the control", () => {
    tour.start("customize-overview");
    ui.go("overview");
    vi.advanceTimersByTime(250);
    expect(tour.index).toBe(1);
    const other = document.createElement("button");
    const edit = document.createElement("button");
    edit.dataset.guide = "dashboard-edit";
    const inner = document.createElement("span");
    edit.append(inner);
    document.body.append(other, edit);
    other.click();
    vi.advanceTimersByTime(200);
    expect(tour.index).toBe(1);
    inner.click();
    vi.advanceTimersByTime(200);
    expect(tour.index).toBe(2);
  });

  it("unknown tours don't start", () => {
    expect(tour.start("nope")).toBe(false);
    expect(tour.tour).toBeNull();
    expect(tourById("first-run")?.steps[0].target).toBeUndefined();
  });
});
