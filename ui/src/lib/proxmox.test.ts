import { describe, expect, it } from "vitest";
import { counts, filterGuests, formatBytes, formatUptime, guestActions, guestTone, isDestructive, isLocal, pct, storageRows, taskLabel, taskTone, usageTone } from "./proxmox";
import { mockProxmox } from "./mockProxmox";
import { backupStorages, backupTargets, backupsByGuest, nextVmid, restoreStorages, scheduleText, snapName, snapTree, validSnapname } from "./proxmox";

describe("proxmox helpers", async () => {
  const o = await mockProxmox.overview(1);

  it("offers actions that fit the state", () => {
    const by = (id: number) => o.guests.find((g) => g.vmid === id)!;
    expect(guestActions(by(100))).toEqual(["shutdown", "reboot", "suspend", "stop"]);
    expect(guestActions(by(200))).toEqual(["shutdown", "reboot", "stop"]);
    expect(guestActions(by(101))).toEqual(["start"]);
    expect(guestActions(by(9000))).toEqual([]);
    expect(guestActions({ ...by(100), status: "paused" })).toEqual(["resume", "stop"]);
    expect(isDestructive("start") || isDestructive("resume")).toBe(false);
    expect(isDestructive("stop") && isDestructive("suspend")).toBe(true);
    expect(guestTone(by(201))).toBe("warn");
    expect(guestTone(by(9000))).toBe("off");
  });

  it("counts and filters guests", () => {
    expect(counts(o.guests)).toEqual({ running: 4, stopped: 2, templates: 1, vms: 4, cts: 2 });
    expect(filterGuests(o.guests, "#dns", null).map((g) => g.vmid)).toEqual([200]);
    expect(filterGuests(o.guests, "ct", null).map((g) => g.vmid)).toEqual([200, 201]);
    expect(filterGuests(o.guests, "", "pve2").length).toBe(2);
    expect(filterGuests(o.guests, "media prod", null).map((g) => g.vmid)).toEqual([100]);
  });

  it("formats sizes, times and usage", () => {
    expect(formatBytes(1024 ** 3 * 1.5)).toBe("1.5 GiB");
    expect(formatBytes(null)).toBe("0 B");
    expect(formatUptime(90061)).toBe("1d 1h");
    expect(formatUptime(3700)).toBe("1h 1m");
    expect(formatUptime(0)).toBe("--");
    expect(pct(50, 200)).toBe(25);
    expect(pct(1, 0)).toBeNull();
    expect([usageTone(95), usageTone(80), usageTone(10), usageTone(null)]).toEqual(["bad", "warn", "ok", "off"]);
  });

  it("merges shared storages and labels tasks", () => {
    const rows = storageRows(o.storages);
    const nas = rows.find((r) => r.s.storage === "nas-backup")!;
    expect(nas.nodes).toEqual(["pve1", "pve2"]);
    expect(rows.filter((r) => r.s.storage === "local-lvm").length).toBe(2);
    const backup = o.tasks.find((t) => t.kind === "vzdump")!;
    expect(taskLabel(backup)).toBe("Backup");
    expect(taskTone(backup)).toBe("bad");
    expect(taskLabel({ ...backup, kind: "qmstart", id: "100" })).toBe("VM start 100");
    expect(taskTone({ ...backup, end: null })).toBe("warn");
    expect(isLocal(o, "pve1")).toBe(true);
    expect(isLocal(o, "pve2")).toBe(false);
  });

  it("mock actions run a task that finishes", async () => {
    const upid = await mockProxmox.action(1, "pve1", "qemu", 101, "start");
    expect(upid).toMatch(/^UPID:pve1:.*:qmstart:101:root@pam:$/);
    const d = await mockProxmox.task(1, "pve1", upid);
    expect(d.running).toBe(true);
    await expect(mockProxmox.action(1, "pve1", "qemu", 100, "start")).rejects.toThrow(/already running/);
  });
});

describe("proxmox snapshots and backups", async () => {
  const o = await mockProxmox.overview(1);

  it("names and orders snapshots", async () => {
    expect(validSnapname("pre_upgrade")).toBe(true);
    expect(validSnapname("1x") || validSnapname("current") || validSnapname("a b")).toBe(false);
    expect(snapName(new Date(2026, 10, 16, 9, 5))).toBe("snap_2026_11_16_0905");
    const tree = snapTree(await mockProxmox.snapshots("pve1", 100));
    expect(tree.map((t) => `${t.depth}:${t.s.name}`)).toEqual(["0:clean_install", "1:jellyfin", "1:pre_upgrade"]);
  });

  it("picks storages and ids", () => {
    expect(backupTargets(o)).toEqual([
      { node: "pve1", storage: "local" },
      { node: "pve1", storage: "nas-backup" },
      { node: "pve2", storage: "nas-backup" },
    ]);
    expect(backupStorages(o, "pve1")).toEqual(["local", "nas-backup"]);
    expect(restoreStorages(o, "pve1", "qemu")).toEqual(["local-lvm"]);
    expect(restoreStorages(o, "pve2", "lxc")).toEqual([]);
    expect(nextVmid(o)).toBe(103);
    expect(scheduleText("02:00")).toBe("daily 02:00");
    expect(scheduleText("sun 03:00")).toBe("sun 03:00");
  });

  it("summarises backups per guest, worst first", async () => {
    const rows = backupsByGuest(o, await mockProxmox.backups());
    expect(rows.slice(0, 3).map((r) => [r.vmid, r.tone])).toEqual([
      [101, "bad"],
      [102, "bad"],
      [201, "bad"],
    ]);
    const media = rows.find((r) => r.vmid === 100)!;
    expect([media.tone, media.files.length, media.covered]).toEqual(["ok", 2, true]);
    expect(rows.find((r) => r.vmid === 101)!.covered).toBe(false);
    expect(rows.some((r) => r.vmid === 9000)).toBe(false);
  });
});

describe("part 3 helpers", () => {
  it("checks guest names, IPs, keys and ISO names", async () => {
    const m = await import("./proxmox");
    expect(m.validGuestName("web-1.lab")).toBe(true);
    expect(m.validGuestName("-bad")).toBe(false);
    expect(m.validGuestName("has space")).toBe(false);
    expect(m.validCtIp("dhcp")).toBe(true);
    expect(m.validCtIp("192.168.1.50/24")).toBe(true);
    expect(m.validCtIp("192.168.1.50")).toBe(false);
    expect(m.validCtIp("192.168.1.300/24")).toBe(false);
    expect(m.validSshKeys("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample you@laptop")).toBe(true);
    expect(m.validSshKeys("")).toBe(false);
    expect(m.validSshKeys("hunter2")).toBe(false);
    expect(m.isoNameFromUrl("https://releases.example.com/24.04/os-24.04-amd64.iso?x=1")).toBe("os-24.04-amd64.iso");
    expect(m.isoNameFromUrl("https://example.com/download")).toBe("");
    expect(m.validIsoName("a b.iso")).toBe(false);
    expect(m.validIsoName("os.iso")).toBe(true);
    expect(m.validSha256("a".repeat(64))).toBe(true);
    expect(m.validUrl("ftp://example.com/x.iso")).toBe(false);
  });
  it("knows which ids are free", async () => {
    const m = await import("./proxmox");
    const o = { guests: [{ vmid: 100 }] } as unknown as Parameters<typeof m.vmidFree>[0];
    expect(m.vmidFree(o, 100)).toBe(false);
    expect(m.vmidFree(o, 101)).toBe(true);
    expect(m.vmidFree(o, 99)).toBe(false);
  });
});

describe("part 4 helpers", () => {
  const cfg = (entries: { key: string; value?: string | null; pending?: string | null; delete?: boolean }[]) =>
    ({ kind: "qemu", digest: "d1", entries: entries.map((e) => ({ value: null, pending: null, delete: false, ...e })), disks: [], nics: [], devices: [], unused: [], pending: false }) as never;

  it("edits options without losing the rest", async () => {
    const m = await import("./proxmox");
    expect(m.withOptions("virtio=AA:BB,bridge=vmbr0,firewall=1", { bridge: "vmbr1", firewall: null, tag: "20" })).toBe("virtio=AA:BB,bridge=vmbr1,tag=20");
    const f = { model: "e1000", bridge: "vmbr0", vlan: null, firewall: true, ip: "dhcp", gateway: "" };
    expect(m.nicValue("qemu", "net0", "virtio=AA:BB,bridge=vmbr1,tag=5,queues=4", f)).toBe("e1000=AA:BB,bridge=vmbr0,queues=4,firewall=1");
    expect(m.nicValue("qemu", "net1", null, f)).toBe("e1000,bridge=vmbr0,firewall=1");
    const ct = { ...f, ip: "192.168.1.9/24", gateway: "192.168.1.1" };
    expect(m.nicValue("lxc", "net1", null, ct)).toBe("name=eth1,bridge=vmbr0,firewall=1,ip=192.168.1.9/24,gw=192.168.1.1");
    expect(m.nicValue("lxc", "net0", "name=eth0,bridge=vmbr0,hwaddr=AA,ip=192.168.1.9/24,gw=192.168.1.1,type=veth", f)).toBe("name=eth0,bridge=vmbr0,hwaddr=AA,ip=dhcp,type=veth,firewall=1");
  });

  it("reads pending values, free keys and only sends changes", async () => {
    const m = await import("./proxmox");
    const c = cfg([{ key: "cores", value: "4", pending: "6" }, { key: "net0", value: "x" }, { key: "net1", value: "y", delete: true }, { key: "onboot", value: "1" }]);
    expect(m.cfgValue(c, "cores")).toBe("6");
    expect(m.cfgValue(c, "net1")).toBeNull();
    expect(m.freeKey(c, "net", 31)).toBe("net2");
    expect(m.changed(c, { cores: "6", onboot: null, memory: "2048" })).toEqual({ set: [{ key: "memory", value: "2048" }], remove: ["onboot"] });
  });

  it("builds disks, devices and startup", async () => {
    const m = await import("./proxmox");
    expect(m.diskValue("qemu", "scsi", "local-lvm", 32)).toBe("local-lvm:32,discard=on,iothread=1");
    expect(m.diskValue("lxc", "", "local-lvm", 16, "/mnt/data")).toBe("local-lvm:16,mp=/mnt/data");
    expect(m.validMountPath("/mnt/../etc")).toBe(false);
    expect(m.validMountPath("/")).toBe(false);
    expect(m.cdromValue(null)).toBe("none,media=cdrom");
    expect(m.pciValue("0000:01:00.0", true, true)).toBe("0000:01:00,pcie=1");
    expect(m.pciValue("0000:01:00.1", false, false)).toBe("0000:01:00.1");
    const pci = [
      { id: "0000:01:00.0", vendor: "", device: "GPU", class: "", iommuGroup: 14 },
      { id: "0000:01:00.1", vendor: "", device: "Audio", class: "", iommuGroup: 14 },
      { id: "0000:02:00.0", vendor: "", device: "NIC", class: "", iommuGroup: 14 },
    ];
    expect(m.groupMates(pci, "0000:01:00.0").map((p) => p.id)).toEqual(["0000:02:00.0"]);
    expect(m.startupOf("order=2,up=30")).toEqual({ order: 2, up: 30 });
    expect(m.startupText(null, null)).toBeNull();
  });
});

describe("part 5 helpers", () => {
  it("names new interfaces and checks specs", async () => {
    const m = await import("./proxmox");
    const ifs = [{ iface: "vmbr0", kind: "bridge" }, { iface: "vmbr1", kind: "bridge" }, { iface: "bond0", kind: "bond" }] as never;
    expect(m.newIface("bridge", ifs).iface).toBe("vmbr2");
    expect(m.newIface("bond", ifs).iface).toBe("bond1");
    expect(m.newIface("vlan", ifs).iface).toBe("vmbr0.10");
    const ok = m.newIface("bridge", ifs);
    expect(m.ifaceProblem(ok)).toBeNull();
    expect(m.ifaceProblem({ ...ok, iface: "br0" })).toMatch(/vmbr/);
    expect(m.ifaceProblem({ ...ok, gateway: "192.168.1.1" })).toMatch(/Gateway/);
    expect(m.ifaceProblem({ ...ok, cidr: "192.168.1.5" })).toMatch(/IPv4/);
    expect(m.ifaceProblem({ ...ok, ports: "enp3s0;reboot" })).toMatch(/Ports/);
    expect(m.ifaceProblem({ ...m.newIface("bond", ifs) })).toMatch(/slave/);
    expect(m.ifaceProblem({ ...m.newIface("vlan", ifs), iface: "vlan30" })).toMatch(/VLAN/);
  });
});

describe("part 6 helpers", () => {
  it("describes and checks firewall rules", async () => {
    const m = await import("./proxmox");
    const r = { ...m.blankRule(), macro: "SSH", source: "+management" };
    expect(m.ruleText(r)).toBe("IN ACCEPT SSH from +management");
    expect(m.ruleText({ ...m.blankRule(), proto: "tcp", dport: "8006", iface: "vmbr0" })).toBe("IN ACCEPT tcp port 8006 on vmbr0");
    expect(m.ruleText({ ...m.blankRule(), kind: "group", action: "webservers" })).toBe("group webservers");
    expect(m.ruleProblem(r)).toBeNull();
    expect(m.ruleProblem({ ...r, action: "ALLOW" })).toMatch(/action/);
    expect(m.ruleProblem({ ...m.blankRule(), dport: "22" })).toMatch(/protocol/);
    expect(m.ruleProblem({ ...r, source: "1.2.3.4; reboot" })).toMatch(/Source/);
    expect(m.ruleSpec({ ...r, pos: 3 })).toEqual(r);
    expect(m.scopeOf("node", { node: "pve1" })).toEqual({ level: "node", node: "pve1", kind: "", vmid: 0, group: "" });
  });
});
