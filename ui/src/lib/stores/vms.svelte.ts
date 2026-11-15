/**
 * Virtual machines and Proxmox containers on the selected System host (M10h):
 * libvirt / KVM via `virsh`, Proxmox VE via `qm` and `pct`. Read without
 * root first; "Read as root" goes through the sudo prompt. Everything but
 * start is confirmed with the guest and host named (rule 5).
 */
import { api, errorText, type Vm, type VmAction, type VmKind, type VmReport } from "../api";
import { system } from "./system.svelte";
import { terms } from "./terms.svelte";

export const VM_KIND_LABEL: Record<VmKind, string> = { libvirt: "KVM", qemu: "PVE VM", lxc: "PVE CT" };

export const VM_ACTION_WORD: Record<VmAction, string> = {
  start: "Start",
  shutdown: "Shut down",
  reboot: "Reboot",
  stop: "Force stop",
};

export const VM_CONSEQUENCE: Partial<Record<VmAction, string>> = {
  shutdown: "The guest is asked to shut down cleanly (ACPI). Guests without an agent or ACPI support may ignore it.",
  reboot: "The guest restarts; its open connections are dropped.",
  stop: "Pulls the plug: the guest stops at once without shutting down. Unsaved data in it is lost.",
};

export type VmTone = "ok" | "warn" | "off";

export function vmTone(v: Vm): VmTone {
  if (v.state === "running") return "ok";
  if (v.state === "stopped") return "off";
  return "warn";
}

/** What makes sense for a guest in its current state. */
export function vmActions(v: Vm): VmAction[] {
  if (v.state === "running") return ["shutdown", "reboot", "stop"];
  if (v.state === "stopped") return ["start"];
  return ["stop"];
}

export const vmKey = (v: Vm): string => `${v.kind}:${v.id}`;

export function vmCounts(r: VmReport | null): { running: number; total: number } {
  const vms = r?.vms ?? [];
  return { running: vms.filter((v) => v.state === "running").length, total: vms.length };
}

class Vms {
  hostId = $state<number | null>(null);
  report = $state<VmReport | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  /** Read (and act) as root on this host. */
  elevated = $state(false);
  busy = $state<string[]>([]);
  pending = $state<{ vm: Vm; action: VmAction } | null>(null);
  notice = $state<{ text: string; tone: "ok" | "err" } | null>(null);
  private seq = 0;

  reset(hostId: number | null) {
    this.seq++;
    this.hostId = hostId;
    this.report = null;
    this.error = null;
    this.notice = null;
    this.pending = null;
    this.busy = [];
    // Root needs no prompt here: read as root right away.
    this.elevated = false;
  }

  /** Root without a typed password (root login, NOPASSWD or remembered). */
  private get freeRoot(): boolean {
    const i = system.info;
    return !!i && (i.sudo === "root" || i.sudo === "noPassword" || i.remembered);
  }

  async load(elevate?: boolean) {
    const id = system.hostId;
    if (id === null) return;
    if (id !== this.hostId) this.reset(id);
    if (elevate !== undefined) this.elevated = elevate;
    else if (!this.elevated && this.freeRoot) this.elevated = true;
    const seq = ++this.seq;
    this.loading = true;
    try {
      const el = this.elevated;
      const r = el ? await system.withSudo("list virtual machines", (sudo) => api.systemVms(id, true, sudo)) : await api.systemVms(id, false, null);
      if (seq !== this.seq) return;
      this.report = r;
      this.error = null;
    } catch (e) {
      if (seq === this.seq) this.error = errorText(e);
    } finally {
      if (seq === this.seq) this.loading = false;
    }
  }

  /** Destructive actions wait for confirmation; start runs at once. */
  request(vm: Vm, action: VmAction) {
    if (action === "start") void this.act(vm, action);
    else this.pending = { vm, action };
  }

  async act(vm: Vm, action: VmAction) {
    const id = this.hostId;
    if (id === null) return;
    this.pending = null;
    const key = vmKey(vm);
    this.busy = [...this.busy, key];
    try {
      // Proxmox tools always need root; libvirt follows how the list was read.
      const root = this.elevated || vm.kind !== "libvirt";
      await (root
        ? system.withSudo(`${action} ${vm.name}`, (sudo) => api.systemVmAction(id, vm.kind, vm.id, action, true, sudo))
        : api.systemVmAction(id, vm.kind, vm.id, action, false, null));
      this.notice = { text: `${VM_ACTION_WORD[action]}: ${vm.name} sent.`, tone: "ok" };
    } catch (e) {
      this.notice = { text: `${VM_ACTION_WORD[action]} ${vm.name}: ${errorText(e)}`, tone: "err" };
    } finally {
      this.busy = this.busy.filter((k) => k !== key);
    }
    if (id === this.hostId) {
      // Shutdowns take a moment: look again shortly after.
      await this.load();
      if (action !== "start") setTimeout(() => id === this.hostId && void this.load(), 4000);
    }
  }

  /** A terminal tab on the guest's console. */
  console(vm: Vm) {
    const id = this.hostId;
    if (id === null) return;
    const asRoot = system.info?.sudo !== "root" && (this.elevated || vm.kind !== "libvirt");
    terms.openVmConsole(id, { kind: vm.kind, id: vm.id, name: vm.name, asRoot });
  }
}

export const vms = new Vms();
