import { describe, expect, it } from "vitest";
import { emptyNav, keyBelongsToTarget, NAV_LIMIT, navButton, navKey, step, SwipeTracker, visit, type NavLoc } from "./navhistory";

const at = (section: string, dims: NavLoc["dims"] = {}): NavLoc => ({ section, dims });

describe("nav history", () => {
  it("records views and tabs and walks back and forward", () => {
    let s = emptyNav();
    s = visit(s, at("overview"));
    s = visit(s, at("fleet", { tab: "updates" }));
    s = visit(s, at("fleet", { tab: "disks" }));
    s = visit(s, at("docker", { tab: "containers", host: 1 }));
    expect(s.back.map((l) => l.section)).toEqual(["overview", "fleet", "fleet"]);

    const b1 = step(s, "back")!;
    expect(b1.to).toEqual(at("fleet", { tab: "disks" }));
    const b2 = step(b1.state, "back")!;
    expect(b2.to).toEqual(at("fleet", { tab: "updates" }));
    const f1 = step(b2.state, "forward")!;
    expect(f1.to).toEqual(at("fleet", { tab: "disks" }));
    const f2 = step(f1.state, "forward")!;
    expect(f2.to.section).toBe("docker");
    expect(step(f2.state, "forward")).toBeNull();
  });

  it("a new visit after going back drops the forward steps", () => {
    let s = visit(visit(visit(emptyNav(), at("overview")), at("files")), at("docker"));
    s = step(s, "back")!.state;
    expect(s.forward).toHaveLength(1);
    s = visit(s, at("system"));
    expect(s.forward).toHaveLength(0);
    expect(s.back.map((l) => l.section)).toEqual(["overview", "files"]);
  });

  it("doesn't add steps for the same place or for a view filling itself in", () => {
    let s = visit(emptyNav(), at("overview"));
    s = visit(s, at("system"));
    // The store loaded and reported its tab: the same place, now known better.
    s = visit(s, at("system", { tab: "units" }));
    // It picked the remembered host by itself.
    s = visit(s, at("system", { tab: "units", host: null }), new Set(["host"]));
    s = visit(s, at("system", { tab: "units", host: 3 }), new Set(["host"]));
    s = visit(s, at("system", { tab: "units", host: 3 }));
    expect(s.back.map((l) => l.section)).toEqual(["overview"]);
    expect(s.current).toEqual(at("system", { tab: "units", host: 3 }));
    // Picking another host is a step.
    s = visit(s, at("system", { tab: "units", host: 4 }), new Set(["host"]));
    expect(s.back).toHaveLength(2);
  });

  it("opening an article from the shelves is a step (not a fill-in)", () => {
    let s = visit(emptyNav(), at("library", { article: null }));
    s = visit(s, at("library", { article: "ssh-basics" }));
    expect(s.back).toEqual([at("library", { article: null })]);
  });

  it("skips places that can't be shown any more", () => {
    let s = visit(visit(visit(emptyNav(), at("overview")), at("proxmox")), at("files"));
    const r = step(s, "back", (l) => l.section !== "proxmox")!;
    expect(r.to.section).toBe("overview");
    s = r.state;
    expect(step(s, "back")).toBeNull();
  });

  it("keeps at most NAV_LIMIT steps back", () => {
    let s = emptyNav();
    for (let i = 0; i < NAV_LIMIT + 20; i++) s = visit(s, at("fleet", { tab: `t${i}` }));
    expect(s.back).toHaveLength(NAV_LIMIT);
  });
});

describe("nav inputs", () => {
  const key = (k: Partial<KeyboardEvent>) => ({ key: "", code: "", ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...k });

  it("maps Cmd+[ / Cmd+] on macOS and Alt+arrows elsewhere", () => {
    expect(navKey(key({ key: "[", metaKey: true }), true)).toBe("back");
    expect(navKey(key({ key: "]", metaKey: true }), true)).toBe("forward");
    // A layout where the key isn't "[" still matches by physical key.
    expect(navKey(key({ key: "ü", code: "BracketLeft", metaKey: true }), true)).toBe("back");
    expect(navKey(key({ key: "ArrowLeft", altKey: true }), true)).toBeNull();
    expect(navKey(key({ key: "ArrowLeft", altKey: true }), false)).toBe("back");
    expect(navKey(key({ key: "ArrowRight", altKey: true }), false)).toBe("forward");
    expect(navKey(key({ key: "ArrowLeft", altKey: true, shiftKey: true }), false)).toBeNull();
    expect(navKey(key({ key: "ArrowLeft", altKey: true, ctrlKey: true }), false)).toBeNull();
    expect(navKey(key({ key: "[", ctrlKey: true }), false)).toBeNull();
    expect(navKey(key({ key: "BrowserBack" }), false)).toBe("back");
    expect(navKey(key({ key: "BrowserForward" }), true)).toBe("forward");
  });

  it("leaves the keys to text fields, VM screens and (off macOS) the terminal", () => {
    const none = { editable: false, terminal: false, vm: false };
    expect(keyBelongsToTarget(none, true)).toBe(false);
    expect(keyBelongsToTarget({ ...none, editable: true }, true)).toBe(true);
    expect(keyBelongsToTarget({ ...none, vm: true }, true)).toBe(true);
    // Shells read Alt+Left/Right; Cmd never reaches them.
    expect(keyBelongsToTarget({ ...none, terminal: true }, false)).toBe(true);
    expect(keyBelongsToTarget({ ...none, terminal: true }, true)).toBe(false);
  });

  it("maps mouse buttons 3 and 4", () => {
    expect(navButton(3)).toBe("back");
    expect(navButton(4)).toBe("forward");
    expect(navButton(0)).toBeNull();
    expect(navButton(1)).toBeNull();
  });

  it("fires once per sideways swipe", () => {
    const t = new SwipeTracker(100, 200);
    let fired: string[] = [];
    for (let i = 0; i < 10; i++) {
      const d = t.feed(-30, 2, i * 16);
      if (d) fired.push(d);
    }
    expect(fired).toEqual(["back"]);
    // Fingers stop, then swipe the other way.
    fired = [];
    for (let i = 0; i < 10; i++) {
      const d = t.feed(30, 0, 1000 + i * 16);
      if (d) fired.push(d);
    }
    expect(fired).toEqual(["forward"]);
  });

  it("ignores mostly vertical scrolling and cancelled gestures", () => {
    const t = new SwipeTracker(100, 200);
    for (let i = 0; i < 20; i++) expect(t.feed(-15, 40, i * 16)).toBeNull();
    const u = new SwipeTracker(100, 200);
    u.cancel(0);
    for (let i = 1; i < 20; i++) expect(u.feed(-40, 0, i * 16)).toBeNull();
  });
});
