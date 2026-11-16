import { describe, expect, it } from "vitest";
import { blank, commandHint, display, intervalLabel, parseThreshold, problems, EXAMPLES } from "./cmdwidgets";
import { add, cmdId, cmdKind, widgetInfo } from "./dashboard";
import { mockCmdWidgets } from "./mockCmdWidgets";

describe("command widgets", () => {
  it("spots commands that write or need sudo", () => {
    expect(commandHint("df -P /")).toBeNull();
    expect(commandHint("sudo zpool status")).toMatch(/sudo/);
    expect(commandHint("docker system prune -f")).toMatch(/only read/);
    expect(commandHint("rm -rf /tmp/x")).toMatch(/only read/);
    expect(commandHint("cat /proc/loadavg; reboot")).toMatch(/only read/);
    expect(commandHint("grep -c down /var/log/x")).toBeNull();
    for (const ex of EXAMPLES) expect(commandHint(ex.w.command ?? ""), ex.label).toBeNull();
  });

  it("checks the form", () => {
    const w = { ...blank(1), name: "Load", command: "cat /proc/loadavg" };
    expect(problems(w)).toEqual({});
    expect(problems({ ...w, name: " " }).name).toBeTruthy();
    expect(problems({ ...w, parse: "regex" }).pattern).toBeTruthy();
    expect(problems({ ...w, parse: "regex", pattern: "(" }).pattern).toBeTruthy();
    expect(problems({ ...w, warn: 90, crit: 80 }).crit).toMatch(/above/);
    expect(problems({ ...w, warn: 20, crit: 10, below: true })).toEqual({});
  });

  it("formats values and thresholds", () => {
    const r = { value: 92, text: "92", level: "crit", exit: 0, output: "", at: 0 };
    expect(display(r, "%")).toBe("92%");
    expect(display({ ...r, value: 0, text: "OK", level: "ok" }, "%")).toBe("OK");
    expect(display({ ...r, value: null, text: "ONLINE", level: "info" }, "")).toBe("ONLINE");
    expect(display(undefined, "%")).toBe("--");
    expect(parseThreshold(" 4.5 ")).toBe(4.5);
    expect(parseThreshold("")).toBeNull();
    expect(parseThreshold("x")).toBeNull();
    expect(intervalLabel(300)).toBe("5 min");
    expect(intervalLabel(120)).toBe("2 min");
  });

  it("places cmd:<id> tiles on the grid", () => {
    expect(cmdKind(7)).toBe("cmd:7");
    expect(cmdId("cmd:7")).toBe(7);
    expect(cmdId("cmd:")).toBeNull();
    expect(cmdId("nodes")).toBeNull();
    expect(widgetInfo("cmd:3")?.title).toBe("COMMAND");
    const ws = add([], cmdKind(3));
    expect(ws[0]).toMatchObject({ kind: "cmd:3", w: 3, h: 2 });
  });

  it("mock runs parse like the backend", async () => {
    const [root] = await mockCmdWidgets.list();
    const r = await mockCmdWidgets.run(root.id);
    expect(r.value).toBeGreaterThan(70);
    expect(["ok", "warn"]).toContain(r.level);
    const t = await mockCmdWidgets.tryRun({ ...blank(1), name: "x", command: "who", parse: "lines" });
    expect(t.value).toBe(2);
    await expect(mockCmdWidgets.save({ ...blank(1), name: "x", command: "sudo ls" })).rejects.toThrow(/sudo/);
  });
});
