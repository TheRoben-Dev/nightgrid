/** Browser-only stand-in for Power & cost (M12d). */
import type { HostPower_Serialize as HostPower, PowerOverview_Serialize as PowerOverview, PowerProfile } from "./bindings";
import { mockBackend } from "./mock";

const profiles = new Map<number, PowerProfile>();
let seeded = false;

export const mockPower = {
  async overview(days: number): Promise<PowerOverview> {
    const hs = await mockBackend.hostsList();
    if (!seeded && hs.length) {
      seeded = true;
      profiles.set(hs[0].id, { hostId: hs[0].id, idleW: 25, maxW: 120 });
      if (hs[1]) profiles.set(hs[1].id, { hostId: hs[1].id, idleW: 7, maxW: 30 });
    }
    const hosts: HostPower[] = hs.map((h, i) => {
      const p = profiles.get(h.id) ?? null;
      const cpu = [12, 4, 30, 2][i % 4];
      const avg = p ? p.idleW + ((p.maxW - p.idleW) * cpu) / 100 : null;
      return {
        hostId: h.id,
        name: h.name,
        profile: p,
        hours: days * 24 - 3,
        avgCpu: cpu,
        gpuW: null,
        avgW: avg,
        peakW: p ? p.idleW + (p.maxW - p.idleW) * 0.9 : null,
        kwhMonth: avg == null ? null : (avg * 730) / 1000,
        idle: p && i === 1 ? [{ from: 23, to: 7, hours: 8, share: 0.93, kwhMonth: ((p.idleW + (p.maxW - p.idleW) * 0.02 - 1) * 8 * 730) / 24 / 1000 }] : [],
      };
    });
    const est = hosts.reduce((a, h) => a + (h.avgW ?? 0), 0);
    return { days, hosts, ups: hs.length ? [{ id: 1, label: "Rack UPS", watts: Math.round(est * 1.12), measured: true, age: 12, hosts: hs.map((h) => h.id), estimatedW: est }] : [] };
  },
  async set(hostId: number, idleW: number, maxW: number): Promise<null> {
    profiles.set(hostId, { hostId, idleW, maxW });
    return null;
  },
  async clear(hostId: number): Promise<null> {
    profiles.delete(hostId);
    return null;
  },
};
