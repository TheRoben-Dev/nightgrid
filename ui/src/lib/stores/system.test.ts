import { describe, expect, it } from "vitest";
import type { JournalEntry, Unit } from "../api";
import { filterJournal, filterUnits, formatHours, isDestructiveUnit, prioLabel, prioTone, system, unitCounts, unitTone } from "./system.svelte";

const unit = (name: string, active: string, sub = "running", description = ""): Unit => ({
  name,
  load: "loaded",
  active,
  sub,
  description,
  fileState: "enabled",
});

const units = [
  unit("nginx.service", "failed", "failed", "web server"),
  unit("ssh.service", "active", "running", "OpenBSD Secure Shell server"),
  unit("ufw.service", "active", "exited", "firewall"),
  unit("rsync.service", "inactive", "dead"),
  unit("foo.service", "activating", "start"),
];

describe("system helpers", () => {
  it("colors and counts units", () => {
    expect(units.map(unitTone)).toEqual(["bad", "ok", "ok", "off", "warn"]);
    expect(unitCounts(units)).toEqual({ running: 1, failed: 1, total: 5 });
  });

  it("filters units by state and text", () => {
    expect(filterUnits(units, "failed", "").map((u) => u.name)).toEqual(["nginx.service"]);
    expect(filterUnits(units, "running", "").length).toBe(2);
    expect(filterUnits(units, "inactive", "").map((u) => u.name)).toEqual(["rsync.service"]);
    expect(filterUnits(units, "all", "SHELL").map((u) => u.name)).toEqual(["ssh.service"]);
  });

  it("only stop, restart and disable need a confirmation", () => {
    expect(["start", "stop", "restart", "reload", "enable", "disable"].filter((a) => isDestructiveUnit(a as never))).toEqual(["stop", "restart", "disable"]);
  });

  it("labels priorities", () => {
    expect([0, 3, 4, 5, 6, 7].map(prioLabel)).toEqual(["EMERG", "ERR", "WARN", "NOTICE", "INFO", "DEBUG"]);
    expect([2, 4, 5, 6, 7].map(prioTone)).toEqual(["bad", "warn", "note", "", "dim"]);
  });

  it("finds journal lines", () => {
    const e = (message: string, ident: string): JournalEntry => ({ ts: 0, priority: 6, unit: `${ident}.service`, ident, pid: 1, message });
    const rows = [e("Accepted publickey", "sshd"), e("container exited", "dockerd")];
    expect(filterJournal(rows, "")).toHaveLength(2);
    expect(filterJournal(rows, "DOCKER").map((r) => r.ident)).toEqual(["dockerd"]);
    expect(filterJournal(rows, "publickey").map((r) => r.ident)).toEqual(["sshd"]);
  });

  it("formats power-on time", () => {
    expect(formatHours(null)).toBe("—");
    expect(formatHours(30)).toBe("30 h");
    expect(formatHours(24 * 10)).toBe("10d");
    expect(formatHours(42411)).toBe("4y 307d");
  });
});

describe("system store (browser mock)", () => {
  it("asks for the sudo password and retries with it", async () => {
    system.select(1);
    await system.loadUnits();
    expect(system.units.length).toBeGreaterThan(3);

    const done = system.act("nginx.service", "restart");
    await new Promise((r) => setTimeout(r, 0));
    expect(system.sudoAsk?.reason).toBe("restart nginx.service");
    expect(system.sudoAsk?.retry).toBe(false);

    // A wrong password asks again, flagged as a retry.
    system.answerSudo({ password: "wrong", remember: false });
    await new Promise((r) => setTimeout(r, 0));
    expect(system.sudoAsk?.retry).toBe(true);

    system.answerSudo({ password: "hunter2", remember: false });
    await done;
    expect(system.sudoAsk).toBeNull();
    expect(system.notice?.tone).toBe("ok");
    expect(system.units.find((u) => u.name === "nginx.service")?.active).toBe("active");
  });

  it("cancelling the prompt fails the action", async () => {
    system.request("ssh.service", "stop");
    expect(system.pendingUnit).toEqual({ unit: "ssh.service", action: "stop" });
    const done = system.act("ssh.service", "stop");
    await new Promise((r) => setTimeout(r, 0));
    system.answerSudo(null);
    await done;
    expect(system.notice?.tone).toBe("err");
    expect(system.units.find((u) => u.name === "ssh.service")?.active).toBe("active");
  });
});
