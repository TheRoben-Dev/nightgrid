import { describe, expect, it } from "vitest";
import { buttonMask, charKeysym, keysymFor, parsePatch, toGuest, typeText } from "./vnc";

function patch(full: boolean, w: number, h: number, rects: { x: number; y: number; w: number; h: number; fill: number }[]): ArrayBuffer {
  const size = 8 + rects.reduce((n, r) => n + 8 + r.w * r.h * 4, 0);
  const buf = new ArrayBuffer(size);
  const v = new DataView(buf);
  v.setUint8(0, full ? 1 : 0);
  v.setUint16(2, w, true);
  v.setUint16(4, h, true);
  v.setUint16(6, rects.length, true);
  let o = 8;
  for (const r of rects) {
    v.setUint16(o, r.x, true);
    v.setUint16(o + 2, r.y, true);
    v.setUint16(o + 4, r.w, true);
    v.setUint16(o + 6, r.h, true);
    o += 8;
    new Uint8Array(buf, o, r.w * r.h * 4).fill(r.fill);
    o += r.w * r.h * 4;
  }
  return buf;
}

describe("vnc patches", () => {
  it("parses rectangles and their pixel offsets", () => {
    const p = parsePatch(patch(false, 800, 600, [{ x: 10, y: 20, w: 2, h: 3, fill: 7 }, { x: 0, y: 0, w: 1, h: 1, fill: 9 }]))!;
    expect(p.full).toBe(false);
    expect([p.width, p.height]).toEqual([800, 600]);
    expect(p.rects).toEqual([
      { x: 10, y: 20, w: 2, h: 3, offset: 16 },
      { x: 0, y: 0, w: 1, h: 1, offset: 16 + 24 + 8 },
    ]);
  });
  it("rejects truncated data and empty buffers", () => {
    const b = patch(true, 4, 4, [{ x: 0, y: 0, w: 4, h: 4, fill: 1 }]);
    expect(parsePatch(b.slice(0, b.byteLength - 1))).toBeNull();
    expect(parsePatch(new ArrayBuffer(0))).toBeNull();
    expect(parsePatch(patch(true, 4, 4, []))?.rects).toEqual([]);
  });
});

describe("vnc input", () => {
  it("maps keys to keysyms", () => {
    expect(keysymFor({ key: "a", code: "KeyA" })).toBe(0x61);
    expect(keysymFor({ key: "A", code: "KeyA" })).toBe(0x41);
    expect(keysymFor({ key: "Enter", code: "Enter" })).toBe(0xff0d);
    expect(keysymFor({ key: "Shift", code: "ShiftRight" })).toBe(0xffe2);
    expect(keysymFor({ key: "Meta", code: "MetaLeft" })).toBe(0xffeb);
    expect(keysymFor({ key: "F5", code: "F5" })).toBe(0xffc2);
    expect(keysymFor({ key: "ArrowUp", code: "ArrowUp" })).toBe(0xff52);
    expect(keysymFor({ key: "ä", code: "Quote" })).toBe(0xe4);
    expect(keysymFor({ key: "€", code: "KeyE" })).toBe(0x010020ac);
    expect(keysymFor({ key: "AltGraph", code: "AltRight" })).toBe(0xfe03);
    expect(keysymFor({ key: "Dead", code: "BracketRight" })).toBeNull();
    expect(keysymFor({ key: "Unidentified", code: "" })).toBeNull();
  });
  it("types text as key presses", () => {
    expect(typeText("a\r\nB")).toEqual([
      { keysym: 0x61, down: true },
      { keysym: 0x61, down: false },
      { keysym: 0xff0d, down: true },
      { keysym: 0xff0d, down: false },
      { keysym: 0x42, down: true },
      { keysym: 0x42, down: false },
    ]);
    expect(charKeysym("\t")).toBe(0xff09);
  });
  it("maps mouse buttons and scaled positions", () => {
    expect(buttonMask(1)).toBe(1);
    expect(buttonMask(2)).toBe(4);
    expect(buttonMask(4)).toBe(2);
    expect(buttonMask(3)).toBe(5);
    const box = { left: 100, top: 50, width: 512, height: 320 };
    expect(toGuest(box, { width: 1024, height: 640 }, 356, 210)).toEqual({ x: 512, y: 320 });
    expect(toGuest(box, { width: 1024, height: 640 }, 0, 0)).toEqual({ x: 0, y: 0 });
    expect(toGuest(box, { width: 1024, height: 640 }, 9999, 9999)).toEqual({ x: 1023, y: 639 });
  });
});
