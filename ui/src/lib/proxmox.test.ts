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
    expect(counts(o.guests)).toEqual({ running: 3, stopped: 2, templates: 1, vms: 3, cts: 2 });
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
