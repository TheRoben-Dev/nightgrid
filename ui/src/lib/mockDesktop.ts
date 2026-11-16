/**
 * Browser-preview stand-in for VM desktops: a fake guest screen (test
 * pattern plus a moving block and a pointer trail) and fake RDP tunnels.
 */
import type { GuestAddrs_Serialize as GuestAddrs, RdpTunnel, VmKind, VncEvent, VncInput } from "./bindings";

const W = 1024;
const H = 640;

interface MockSession {
  on: (e: VncEvent) => void;
  px: Uint8ClampedArray;
  dirty: { x: number; y: number; w: number; h: number }[];
  full: boolean;
  closed: boolean;
  tick: number;
  waiters: (() => void)[];
}

const sessions = new Map<number, MockSession>();
const tunnels = new Map<number, RdpTunnel>();
let next = 0;

function fill(s: MockSession, x: number, y: number, w: number, h: number, rgb: [number, number, number]) {
  for (let j = y; j < Math.min(H, y + h); j++) {
    for (let i = x; i < Math.min(W, x + w); i++) {
      const o = (j * W + i) * 4;
      s.px[o] = rgb[0];
      s.px[o + 1] = rgb[1];
      s.px[o + 2] = rgb[2];
      s.px[o + 3] = 255;
    }
  }
  s.dirty.push({ x, y, w: Math.min(w, W - x), h: Math.min(h, H - y) });
  for (const w of s.waiters.splice(0)) w();
}

function pattern(s: MockSession) {
  fill(s, 0, 0, W, H, [14, 22, 30]);
  const bars: [number, number, number][] = [
    [235, 235, 235],
    [230, 210, 60],
    [60, 210, 220],
    [70, 200, 120],
    [200, 70, 200],
    [210, 60, 60],
    [50, 70, 200],
  ];
  bars.forEach((c, i) => fill(s, 112 + i * 112, 120, 100, 260, c));
  fill(s, 0, H - 40, W, 40, [24, 34, 44]);
  fill(s, 8, H - 32, 80, 24, [70, 200, 160]);
}

function patch(s: MockSession): ArrayBuffer {
  const rects = s.full ? [{ x: 0, y: 0, w: W, h: H }] : s.dirty.splice(0);
  s.full = false;
  s.dirty = [];
  const size = 8 + rects.reduce((n, r) => n + 8 + r.w * r.h * 4, 0);
  const buf = new ArrayBuffer(size);
  const v = new DataView(buf);
  const bytes = new Uint8ClampedArray(buf);
  v.setUint8(0, rects.length === 1 && rects[0].w === W && rects[0].h === H ? 1 : 0);
  v.setUint16(2, W, true);
  v.setUint16(4, H, true);
  v.setUint16(6, rects.length, true);
  let o = 8;
  for (const r of rects) {
    v.setUint16(o, r.x, true);
    v.setUint16(o + 2, r.y, true);
    v.setUint16(o + 4, r.w, true);
    v.setUint16(o + 6, r.h, true);
    o += 8;
    for (let j = r.y; j < r.y + r.h; j++) {
      const from = (j * W + r.x) * 4;
      bytes.set(s.px.subarray(from, from + r.w * 4), o);
      o += r.w * 4;
    }
  }
  return buf;
}

export const mockDesktop = {
  async open(kind: VmKind, id: string, on: (e: VncEvent) => void): Promise<number> {
    await new Promise((r) => setTimeout(r, 300));
    if (id === "broken") throw new Error("this VM's display is SPICE: switch its graphics to VNC to view it here");
    const sid = ++next;
    const s: MockSession = { on, px: new Uint8ClampedArray(W * H * 4), dirty: [], full: true, closed: false, tick: 0, waiters: [] };
    sessions.set(sid, s);
    pattern(s);
    s.full = true;
    on({ type: "connected", name: `${kind === "libvirt" ? id : `VM ${id}`} (preview)`, width: W, height: H });
    const timer = setInterval(() => {
      if (s.closed) return clearInterval(timer);
      s.tick++;
      const x = 120 + ((s.tick * 8) % 760);
      fill(s, 112, 420, 800, 40, [14, 22, 30]);
      fill(s, x, 424, 32, 32, [255, 120, 200]);
    }, 120);
    return sid;
  },
  async frame(sid: number): Promise<ArrayBuffer> {
    const s = sessions.get(sid);
    if (!s || s.closed) return new ArrayBuffer(0);
    if (!s.full && !s.dirty.length) {
      await new Promise<void>((r) => {
        s.waiters.push(r);
        setTimeout(r, 1000);
      });
    }
    if (s.closed || (!s.full && !s.dirty.length)) return new ArrayBuffer(0);
    return patch(s);
  },
  async input(sid: number, events: VncInput[]): Promise<null> {
    const s = sessions.get(sid);
    if (!s) throw new Error("this desktop is closed");
    for (const e of events) {
      if (e.type === "pointer" && e.buttons) fill(s, e.x - 2, e.y - 2, 5, 5, [70, 200, 160]);
      if (e.type === "key" && e.down) {
        fill(s, 96, H - 32, 200, 24, [24, 34, 44]);
        fill(s, 96 + (e.keysym % 25) * 8, H - 30, 6, 20, [230, 210, 60]);
      }
      if (e.type === "refresh") s.full = true;
    }
    return null;
  },
  close(sid: number): void {
    const s = sessions.get(sid);
    if (!s) return;
    s.closed = true;
    sessions.delete(sid);
    for (const w of s.waiters.splice(0)) w();
  },
  async guestAddrs(kind: VmKind, id: string): Promise<GuestAddrs> {
    await new Promise((r) => setTimeout(r, 200));
    if (kind === "qemu" && id.endsWith("2")) return { addrs: [], note: "the QEMU guest agent isn't running in the VM: enter the address yourself" };
    return { addrs: ["192.168.122.45"], note: null };
  },
  async rdpOpen(hostId: number, guest: string, addr: string): Promise<RdpTunnel> {
    const t: RdpTunnel = { id: ++next, hostId, guest, addr, localPort: 50000 + next };
    tunnels.set(t.id, t);
    return t;
  },
  rdpList: async (): Promise<RdpTunnel[]> => [...tunnels.values()],
  rdpClose(id: number): void {
    tunnels.delete(id);
  },
};
