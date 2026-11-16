/**
 * VM desktops (M10n): guest screens open in the Desktops view, one tab each,
 * plus RDP tunnels for Windows guests. Opened from System → VMS.
 */
import { api, errorText, type GuestAddrs, type RdpTunnel, type Vm, type VncEvent } from "../api";
import { system } from "./system.svelte";
import { ui } from "./ui.svelte";
import { vms } from "./vms.svelte";

export type DesktopStatus = "connecting" | "connected" | "closed";

export interface DesktopTab {
  /** Local tab id (stable across reconnects). */
  key: number;
  session: number | null;
  hostId: number;
  hostName: string;
  vm: Pick<Vm, "kind" | "id" | "name">;
  /** Read virsh as root. */
  elevate: boolean;
  status: DesktopStatus;
  error: string | null;
  title: string;
  width: number;
  height: number;
  /** Text the guest last put on its clipboard. */
  clipboard: string | null;
}

export interface RdpAsk {
  hostId: number;
  hostName: string;
  vm: Pick<Vm, "kind" | "id" | "name">;
  elevate: boolean;
  loading: boolean;
  found: GuestAddrs | null;
  addr: string;
  port: number;
  error: string | null;
  busy: boolean;
}

export const desktopLabel = (t: Pick<DesktopTab, "vm" | "hostName">): string => `${t.vm.name} · ${t.hostName}`;

class Desktops {
  tabs = $state<DesktopTab[]>([]);
  active = $state<number | null>(null);
  rdp = $state<RdpTunnel[]>([]);
  rdpAsk = $state<RdpAsk | null>(null);
  private next = 0;

  get current(): DesktopTab | null {
    return this.tabs.find((t) => t.key === this.active) ?? null;
  }

  private patch(key: number, p: Partial<DesktopTab>) {
    this.tabs = this.tabs.map((t) => (t.key === key ? { ...t, ...p } : t));
  }

  private onEvent(key: number, e: VncEvent) {
    switch (e.type) {
      case "connected":
        this.patch(key, { status: "connected", error: null, title: e.name, width: e.width, height: e.height });
        break;
      case "clipboard":
        this.patch(key, { clipboard: e.text });
        break;
      case "closed":
        this.patch(key, { status: "closed", session: null, error: e.reason || null });
        break;
      case "bell":
        break;
    }
  }

  /** Opens (or focuses) a guest's screen from the VMS panel. */
  async open(vm: Vm) {
    const hostId = system.hostId;
    if (hostId === null) return;
    const existing = this.tabs.find((t) => t.hostId === hostId && t.vm.kind === vm.kind && t.vm.id === vm.id);
    ui.go("desktops");
    if (existing) {
      this.active = existing.key;
      if (existing.status === "closed") await this.connect(existing.key);
      return;
    }
    const key = ++this.next;
    const tab: DesktopTab = {
      key,
      session: null,
      hostId,
      hostName: system.host?.name ?? `host ${hostId}`,
      vm: { kind: vm.kind, id: vm.id, name: vm.name },
      elevate: vms.elevated,
      status: "connecting",
      error: null,
      title: vm.name,
      width: 0,
      height: 0,
      clipboard: null,
    };
    this.tabs = [...this.tabs, tab];
    this.active = key;
    await this.connect(key);
  }

  async connect(key: number) {
    const t = this.tabs.find((x) => x.key === key);
    if (!t) return;
    this.patch(key, { status: "connecting", error: null });
    const root = t.elevate || t.vm.kind !== "libvirt";
    try {
      const sid = root
        ? await system.withSudo(`open the screen of ${t.vm.name}`, (sudo) => api.desktopOpen(t.hostId, t.vm.kind, t.vm.id, true, sudo, (e) => this.onEvent(key, e)), t.hostName)
        : await api.desktopOpen(t.hostId, t.vm.kind, t.vm.id, false, null, (e) => this.onEvent(key, e));
      if (!this.tabs.some((x) => x.key === key)) {
        void api.desktopClose(sid);
        return;
      }
      this.patch(key, { session: sid });
    } catch (e) {
      const msg = errorText(e);
      this.patch(key, { status: "closed", error: msg === "Cancelled." ? "Not opened: root is needed to reach this display." : msg });
    }
  }

  disconnect(key: number) {
    const t = this.tabs.find((x) => x.key === key);
    if (t?.session != null) void api.desktopClose(t.session);
    this.patch(key, { status: "closed", session: null, error: null });
  }

  close(key: number) {
    const t = this.tabs.find((x) => x.key === key);
    if (t?.session != null) void api.desktopClose(t.session);
    const i = this.tabs.findIndex((x) => x.key === key);
    this.tabs = this.tabs.filter((x) => x.key !== key);
    if (this.active === key) this.active = this.tabs[Math.min(i, this.tabs.length - 1)]?.key ?? null;
  }

  /** RDP: look up the guest's address, then ask for confirmation. */
  async askRdp(vm: Vm) {
    const hostId = system.hostId;
    if (hostId === null) return;
    const elevate = vms.elevated;
    const ask: RdpAsk = {
      hostId,
      hostName: system.host?.name ?? `host ${hostId}`,
      vm: { kind: vm.kind, id: vm.id, name: vm.name },
      elevate,
      loading: true,
      found: null,
      addr: "",
      port: 3389,
      error: null,
      busy: false,
    };
    this.rdpAsk = ask;
    try {
      const root = elevate || vm.kind !== "libvirt";
      const found = root
        ? await system.withSudo(`find the address of ${vm.name}`, (sudo) => api.desktopGuestAddrs(hostId, vm.kind, vm.id, true, sudo), ask.hostName)
        : await api.desktopGuestAddrs(hostId, vm.kind, vm.id, false, null);
      if (this.rdpAsk?.vm.id !== vm.id) return;
      this.rdpAsk = { ...this.rdpAsk, loading: false, found, addr: found.addrs[0] ?? "" };
    } catch (e) {
      if (this.rdpAsk?.vm.id !== vm.id) return;
      const msg = errorText(e);
      this.rdpAsk = { ...this.rdpAsk, loading: false, error: msg === "Cancelled." ? null : msg };
    }
  }

  async startRdp() {
    const a = this.rdpAsk;
    if (!a || !a.addr.trim()) return;
    this.rdpAsk = { ...a, busy: true, error: null };
    try {
      const t = await api.desktopRdpOpen(a.hostId, `${a.vm.name} · ${a.hostName}`, a.addr.trim(), a.port || null);
      this.rdp = [...this.rdp.filter((x) => x.id !== t.id), t];
      this.rdpAsk = null;
    } catch (e) {
      if (this.rdpAsk) this.rdpAsk = { ...this.rdpAsk, busy: false, error: errorText(e) };
      await this.loadRdp();
    }
  }

  async loadRdp() {
    try {
      this.rdp = await api.desktopRdpList();
    } catch {
      /* keep the last list */
    }
  }

  async stopRdp(id: number) {
    await api.desktopRdpClose(id);
    this.rdp = this.rdp.filter((t) => t.id !== id);
  }
}

export const desktops = new Desktops();
