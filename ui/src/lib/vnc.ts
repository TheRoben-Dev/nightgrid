/**
 * Helpers for the built-in VNC viewer (M10n): drawing the binary patches the
 * backend sends, and turning keyboard and mouse events into RFB input.
 */

export interface PatchRect {
  x: number;
  y: number;
  w: number;
  h: number;
  /** Byte offset of the rectangle's RGBA pixels in the patch. */
  offset: number;
}

export interface Patch {
  full: boolean;
  width: number;
  height: number;
  rects: PatchRect[];
}

/** Reads a patch header (layout in core/src/vnc/frame.rs, little-endian). */
export function parsePatch(buf: ArrayBuffer): Patch | null {
  if (buf.byteLength < 8) return null;
  const v = new DataView(buf);
  const p: Patch = { full: v.getUint8(0) === 1, width: v.getUint16(2, true), height: v.getUint16(4, true), rects: [] };
  const n = v.getUint16(6, true);
  let o = 8;
  for (let i = 0; i < n; i++) {
    if (o + 8 > buf.byteLength) return null;
    const r = { x: v.getUint16(o, true), y: v.getUint16(o + 2, true), w: v.getUint16(o + 4, true), h: v.getUint16(o + 6, true), offset: o + 8 };
    o = r.offset + r.w * r.h * 4;
    if (o > buf.byteLength) return null;
    p.rects.push(r);
  }
  return p;
}

/** Draws a patch; resizes the canvas when the guest's screen size changed. */
export function drawPatch(canvas: HTMLCanvasElement, buf: ArrayBuffer): Patch | null {
  const p = parsePatch(buf);
  if (!p) return null;
  if (canvas.width !== p.width || canvas.height !== p.height) {
    canvas.width = p.width;
    canvas.height = p.height;
  }
  const ctx = canvas.getContext("2d");
  if (!ctx) return p;
  for (const r of p.rects) {
    if (!r.w || !r.h) continue;
    const px = new Uint8ClampedArray(buf, r.offset, r.w * r.h * 4);
    ctx.putImageData(new ImageData(px, r.w, r.h), r.x, r.y);
  }
  return p;
}

/** X11 keysyms for keys that don't type a character, by KeyboardEvent.code / key. */
const NAMED: Record<string, number> = {
  Backspace: 0xff08,
  Tab: 0xff09,
  Enter: 0xff0d,
  NumpadEnter: 0xff8d,
  Escape: 0xff1b,
  Delete: 0xffff,
  Home: 0xff50,
  ArrowLeft: 0xff51,
  ArrowUp: 0xff52,
  ArrowRight: 0xff53,
  ArrowDown: 0xff54,
  PageUp: 0xff55,
  PageDown: 0xff56,
  End: 0xff57,
  Insert: 0xff63,
  ContextMenu: 0xff67,
  Pause: 0xff13,
  ScrollLock: 0xff14,
  PrintScreen: 0xff61,
  NumLock: 0xff7f,
  CapsLock: 0xffe5,
  ShiftLeft: 0xffe1,
  ShiftRight: 0xffe2,
  ControlLeft: 0xffe3,
  ControlRight: 0xffe4,
  AltLeft: 0xffe9,
  AltRight: 0xffea,
  MetaLeft: 0xffeb,
  MetaRight: 0xffec,
  OSLeft: 0xffeb,
  OSRight: 0xffec,
};

export const KEYSYM = { ctrl: 0xffe3, alt: 0xffe9, del: 0xffff, shift: 0xffe1, superL: 0xffeb } as const;

/** The keysym for a typed character (Latin-1 directly, the rest as Unicode keysyms). */
export function charKeysym(ch: string): number {
  const cp = ch.codePointAt(0) ?? 0;
  if (ch === "\n") return 0xff0d;
  if (ch === "\t") return 0xff09;
  if (cp >= 0x20 && cp <= 0x7e) return cp;
  if (cp >= 0xa0 && cp <= 0xff) return cp;
  return 0x01000000 + cp;
}

/** The keysym for a key event, or null for keys to ignore (dead keys, IME). */
export function keysymFor(e: Pick<KeyboardEvent, "key" | "code">): number | null {
  if (e.key === "AltGraph") return 0xfe03;
  const named = NAMED[e.code] ?? NAMED[e.key];
  if (named !== undefined) return named;
  const fn = /^F(\d{1,2})$/.exec(e.key);
  if (fn) {
    const n = Number(fn[1]);
    if (n >= 1 && n <= 24) return 0xffbe + n - 1;
  }
  if (e.key === "Dead" || e.key === "Process" || e.key === "Unidentified") return null;
  if ([...e.key].length === 1) return charKeysym(e.key);
  return null;
}

/** Key presses that type `text` (for passwords in installers, or guests without a clipboard agent). */
export function typeText(text: string): { keysym: number; down: boolean }[] {
  const out: { keysym: number; down: boolean }[] = [];
  for (const ch of text.replace(/\r\n?/g, "\n")) {
    const k = charKeysym(ch);
    out.push({ keysym: k, down: true }, { keysym: k, down: false });
  }
  return out;
}

/** RFB button mask from MouseEvent.buttons (left 1, middle 2, right 4). */
export function buttonMask(buttons: number): number {
  let m = 0;
  if (buttons & 1) m |= 1;
  if (buttons & 4) m |= 2;
  if (buttons & 2) m |= 4;
  return m;
}

/** A client point mapped onto the guest's screen (the canvas may be scaled). */
export function toGuest(
  box: { left: number; top: number; width: number; height: number },
  fb: { width: number; height: number },
  clientX: number,
  clientY: number,
): { x: number; y: number } {
  const sx = box.width ? fb.width / box.width : 1;
  const sy = box.height ? fb.height / box.height : 1;
  const clamp = (v: number, max: number) => Math.max(0, Math.min(max - 1, Math.floor(v)));
  return { x: clamp((clientX - box.left) * sx, fb.width), y: clamp((clientY - box.top) * sy, fb.height) };
}
