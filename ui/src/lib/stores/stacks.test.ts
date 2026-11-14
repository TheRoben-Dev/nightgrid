import { describe, expect, it } from "vitest";
import { docker } from "./docker.svelte";
import { composeFile, envFile, isDestructiveOp, stacks } from "./stacks.svelte";

const until = async (cond: () => boolean, ms = 5000) => {
  const end = Date.now() + ms;
  while (!cond()) {
    if (Date.now() > end) throw new Error("timed out");
    await new Promise((r) => setTimeout(r, 20));
  }
};

describe("stacks store", () => {
  it("lists stacks from container labels, then from stack folders", async () => {
    docker.select(7);
    await stacks.load();
    expect(stacks.stacks.map((s) => s.name)).toEqual(["dash", "media"]);
    const media = stacks.stacks.find((s) => s.name === "media")!;
    expect(media.running).toBe(2);
    expect(media.total).toBe(3);
    expect(composeFile(media)).toBe("/srv/media/compose.yaml");
    expect(envFile(media)).toBe("/srv/media/.env");

    expect(await stacks.addDir("/opt/stacks")).toBeNull();
    expect(stacks.dirs.map((d) => d.path)).toEqual(["/opt/stacks"]);
    expect(stacks.stacks.map((s) => s.name)).toEqual(["dash", "media", "vault"]);
    await stacks.removeDir(stacks.dirs[0].id);
    expect(stacks.dirs).toHaveLength(0);
  });

  it("asks before down/restart/update and streams the run", async () => {
    const media = stacks.stacks.find((s) => s.name === "media")!;
    stacks.request(media, "down");
    expect(stacks.pending?.stack.name).toBe("media");
    expect(stacks.run).toBeNull();
    await stacks.start(stacks.pending!.stack, stacks.pending!.op);
    expect(stacks.pending).toBeNull();
    expect(stacks.busy).toBe(true);
    await until(() => !stacks.busy);
    expect(stacks.run?.code).toBe(0);
    expect(stacks.run?.lines.some((l) => l.includes("Removed"))).toBe(true);
  });

  it("reports image changes after update; up and pull run without asking", async () => {
    expect(isDestructiveOp("up")).toBe(false);
    expect(isDestructiveOp("pull")).toBe(false);
    const media = stacks.stacks.find((s) => s.name === "media")!;
    stacks.request(media, "pull");
    expect(stacks.pending).toBeNull();
    await until(() => !stacks.busy);
    await stacks.start(media, "update");
    await until(() => !stacks.busy);
    expect(stacks.run?.changes?.length).toBe(1);
  });
});
