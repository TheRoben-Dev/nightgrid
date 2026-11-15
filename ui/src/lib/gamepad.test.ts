import { describe, expect, it } from "vitest";
import { BTN, PadReader, pickNext, type Box } from "./gamepad";

const b = (x: number, y: number, w = 100, h = 30): Box => ({ x, y, w, h });

describe("spatial focus", () => {
  // A 2x2 grid of buttons plus a wide one below.
  const grid = [b(0, 0), b(200, 0), b(0, 100), b(200, 100), b(0, 200, 300)];

  it("moves within the column first", () => {
    expect(pickNext(grid[0], grid, "down")).toBe(2);
    expect(pickNext(grid[1], grid, "down")).toBe(3);
    expect(pickNext(grid[3], grid, "down")).toBe(4);
    expect(pickNext(grid[2], grid, "up")).toBe(0);
  });

  it("moves sideways in the row", () => {
    expect(pickNext(grid[0], grid, "right")).toBe(1);
    expect(pickNext(grid[3], grid, "left")).toBe(2);
  });

  it("returns -1 at the edge", () => {
    expect(pickNext(grid[0], grid, "up")).toBe(-1);
    expect(pickNext(grid[1], grid, "right")).toBe(-1);
  });
});

describe("pad reader", () => {
  const btns = (...on: number[]) => Array.from({ length: 17 }, (_, i) => on.includes(i));

  it("fires presses once and repeats held directions", () => {
    const r = new PadReader();
    expect(r.read(btns(BTN.A), [0, 0], 0)).toEqual([{ kind: "activate" }]);
    expect(r.read(btns(BTN.A), [0, 0], 16)).toEqual([]);
    expect(r.read(btns(BTN.DOWN), [0, 0], 32)).toEqual([{ kind: "move", dir: "down" }]);
    expect(r.read(btns(BTN.DOWN), [0, 0], 200)).toEqual([]);
    expect(r.read(btns(BTN.DOWN), [0, 0], 420)).toEqual([{ kind: "move", dir: "down" }]);
    expect(r.read(btns(BTN.DOWN), [0, 0], 450)).toEqual([]);
    expect(r.read(btns(BTN.DOWN), [0, 0], 540)).toEqual([{ kind: "move", dir: "down" }]);
  });

  it("reads the stick and the shoulder buttons", () => {
    const r = new PadReader();
    expect(r.read(btns(), [0.9, 0], 0)).toEqual([{ kind: "move", dir: "right" }]);
    expect(r.read(btns(), [0.2, 0], 10)).toEqual([]);
    expect(r.read(btns(BTN.RB, BTN.B), [0, 0], 20)).toEqual([{ kind: "back" }, { kind: "view", step: 1 }]);
    expect(r.read(btns(BTN.START), [0, 0], 30)).toEqual([{ kind: "palette" }]);
  });
});

describe("moveFocus in the page", () => {
  it("moves focus to the button below and stays inside an open dialog", async () => {
    const { moveFocus } = await import("./gamepad");
    document.body.innerHTML = `<button id="a"></button><button id="b"></button><dialog open><button id="c"></button><button id="d"></button></dialog>`;
    const rect = (x: number, y: number) => () => ({ left: x, top: y, right: x + 50, bottom: y + 20, width: 50, height: 20, x, y, toJSON: () => ({}) }) as DOMRect;
    const pos: Record<string, [number, number]> = { a: [0, 0], b: [0, 50], c: [100, 0], d: [100, 50] };
    for (const [id, [x, y]] of Object.entries(pos)) {
      const el = document.getElementById(id)!;
      el.getBoundingClientRect = rect(x, y);
      el.scrollIntoView = () => {};
    }
    document.getElementById("c")!.focus();
    moveFocus("down");
    expect(document.activeElement?.id).toBe("d");
    // a and b are outside the dialog, so left finds nothing.
    moveFocus("left");
    expect(document.activeElement?.id).toBe("d");
    document.querySelector("dialog")!.removeAttribute("open");
    document.getElementById("a")!.focus();
    moveFocus("down");
    expect(document.activeElement?.id).toBe("b");
  });
});
