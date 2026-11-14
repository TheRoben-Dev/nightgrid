import { describe, expect, it } from "vitest";
import { actionLabel, docker, isDestructive } from "./docker.svelte";

describe("docker store", () => {
  it("groups containers by compose project, standalone last", async () => {
    docker.select(1);
    await docker.refresh();
    expect(docker.loaded).toBe(true);
    expect(docker.groups.map((g) => g.label)).toEqual(["dash", "media", "standalone"]);
    const media = docker.groups.find((g) => g.key === "media")!;
    expect(media.dir).toBe("/srv/media");
    expect(media.running).toBe(2);
    expect(media.containers).toHaveLength(3);
  });

  it("filters and hides stopped containers", async () => {
    docker.filter = "sonarr";
    expect(docker.visible.map((c) => c.name)).toEqual(["media-sonarr-1"]);
    docker.showStopped = false;
    expect(docker.visible).toHaveLength(0);
    docker.filter = "";
    docker.showStopped = true;
  });

  it("asks before destructive actions and names the targets", async () => {
    const plex = docker.containers.find((c) => c.name === "plex")!;
    docker.request({ kind: "stop" }, [plex]);
    expect(docker.pending?.targets.map((t) => t.name)).toEqual(["plex"]);
    await docker.run(docker.pending!.action, docker.pending!.targets);
    expect(docker.pending).toBeNull();
    expect(docker.containers.find((c) => c.name === "plex")?.state).toBe("exited");
    // Start is not destructive: runs straight away.
    docker.request({ kind: "start" }, [plex]);
    expect(docker.pending).toBeNull();
  });

  it("labels actions", () => {
    expect(actionLabel({ kind: "remove", force: true })).toBe("Force remove");
    expect(actionLabel({ kind: "restart" })).toBe("Restart");
    expect(isDestructive({ kind: "kill" })).toBe(true);
    expect(isDestructive({ kind: "unpause" })).toBe(false);
  });
});
