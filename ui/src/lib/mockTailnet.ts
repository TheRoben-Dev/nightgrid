/** Browser-only stand-in for the built-in Tailscale client (M10j). */
import type { TailnetStatus_Serialize as TailnetStatus } from "./bindings";

let s: TailnetStatus = {
  state: "off",
  loginUrl: null,
  tailnet: null,
  magicDnsSuffix: null,
  selfNode: null,
  peers: [],
  acceptRoutes: true,
  error: null,
};
const subs = new Set<(s: TailnetStatus) => void>();
const set = (p: Partial<TailnetStatus>) => {
  s = { ...s, ...p };
  subs.forEach((f) => f(s));
};

const running = (): Partial<TailnetStatus> => ({
  state: "running",
  loginUrl: null,
  tailnet: "you@example.com",
  magicDnsSuffix: "tail1234.ts.net",
  selfNode: { name: "nightgrid", dnsName: "nightgrid.tail1234.ts.net", ips: ["100.70.3.9"], os: "macOS", online: true, routes: [] },
  peers: [
    { name: "lumen", dnsName: "lumen.tail1234.ts.net", ips: ["100.101.7.12", "fd7a:115c:a1e0::5"], os: "linux", online: true, routes: ["192.168.1.0/24"] },
    { name: "phone", dnsName: "phone.tail1234.ts.net", ips: ["100.88.1.9"], os: "iOS", online: false, routes: [] },
  ],
});

export const mockTailnet = {
  subscribe(f: (s: TailnetStatus) => void): () => void {
    subs.add(f);
    return () => subs.delete(f);
  },
  status: async (): Promise<TailnetStatus> => s,
  setEnabled: async (on: boolean): Promise<TailnetStatus> => {
    if (!on) set({ ...running(), state: "off", peers: [], selfNode: null, tailnet: null });
    else {
      set({ state: "starting" });
      setTimeout(() => set({ state: "needsLogin" }), 600);
    }
    return s;
  },
  configure: async (acceptRoutes: boolean): Promise<TailnetStatus> => {
    set({ acceptRoutes });
    return s;
  },
  login: async (): Promise<null> => {
    set({ loginUrl: "https://login.tailscale.com/a/example" });
    setTimeout(() => set(running()), 2500);
    return null;
  },
  logout: async (): Promise<null> => {
    set({ state: "needsLogin", peers: [], selfNode: null, tailnet: null });
    return null;
  },
};
