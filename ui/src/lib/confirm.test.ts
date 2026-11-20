import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { enterConfirms, isConfirmEnter, typedMatches } from "./confirm";

describe("typedMatches", () => {
  it("ignores case and surrounding spaces", () => {
    expect(typedMatches("BRAINCAGE", "braincage")).toBe(true);
    expect(typedMatches("  braincage ", "braincage")).toBe(true);
    expect(typedMatches("Lumen", "LUMEN")).toBe(true);
  });
  it("still needs the whole name", () => {
    expect(typedMatches("brain", "braincage")).toBe(false);
    expect(typedMatches("", "braincage")).toBe(false);
    expect(typedMatches("braincage2", "braincage")).toBe(false);
  });
  it("never matches a missing or empty name", () => {
    expect(typedMatches("", "")).toBe(false);
    expect(typedMatches("x", null)).toBe(false);
    expect(typedMatches("x", undefined)).toBe(false);
  });
});

function setup(enabled: boolean) {
  document.body.innerHTML = `<dialog open><input id="t" /><button data-confirm ${enabled ? "" : "disabled"}>Go</button></dialog>`;
  const input = document.getElementById("t") as HTMLInputElement;
  const btn = document.querySelector("button") as HTMLButtonElement;
  let clicks = 0;
  btn.addEventListener("click", () => clicks++);
  const action = enterConfirms(input);
  const press = (init: KeyboardEventInit) => {
    const e = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init });
    input.dispatchEvent(e);
    return e;
  };
  return { press, clicks: () => clicks, action };
}

describe("enterConfirms", () => {
  it("Enter and keypad Enter click the enabled confirm button", () => {
    const t = setup(true);
    t.press({ key: "Enter", code: "Enter" });
    t.press({ key: "Enter", code: "NumpadEnter" });
    expect(t.clicks()).toBe(2);
  });
  it("never confirms while the button is disabled (name doesn't match)", () => {
    const t = setup(false);
    const e = t.press({ key: "Enter", code: "Enter" });
    expect(t.clicks()).toBe(0);
    expect(e.defaultPrevented).toBe(true);
  });
  it("ignores IME composition, modifiers, repeats and other keys", () => {
    const t = setup(true);
    t.press({ key: "Enter", isComposing: true });
    t.press({ key: "Process", keyCode: 229 });
    t.press({ key: "Enter", shiftKey: true });
    t.press({ key: "Enter", repeat: true });
    t.press({ key: "a" });
    expect(t.clicks()).toBe(0);
  });
  it("stops listening when destroyed", () => {
    const t = setup(true);
    t.action.destroy();
    t.press({ key: "Enter" });
    expect(t.clicks()).toBe(0);
  });
  it("isConfirmEnter accepts plain Enter only", () => {
    expect(isConfirmEnter(new KeyboardEvent("keydown", { key: "Enter" }))).toBe(true);
    expect(isConfirmEnter(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true }))).toBe(false);
  });
});

describe("every type-to-confirm dialog confirms with Enter", () => {
  const root = join(__dirname, "..");
  const files: string[] = [];
  const walk = (d: string) => {
    for (const n of readdirSync(d)) {
      const p = join(d, n);
      if (statSync(p).isDirectory()) walk(p);
      else if (p.endsWith(".svelte")) files.push(p);
    }
  };
  walk(root);
  const users = files.filter((f) => readFileSync(f, "utf8").includes("typedMatches("));
  it("finds the dialogs", () => expect(users.length).toBeGreaterThanOrEqual(11));
  for (const f of users) {
    it(f.slice(root.length + 1), () => {
      const src = readFileSync(f, "utf8");
      // Either a <form> whose submit button is disabled until the name matches,
      // or use:enterConfirms on the input plus data-confirm on the button.
      const viaForm = /<form[^>]*onsubmit/.test(src) && /type="submit"[^>]*disabled=/.test(src);
      const viaAction = src.includes("use:enterConfirms") && /data-confirm(\s|>)/.test(src);
      expect(viaForm || viaAction).toBe(true);
    });
  }
});
