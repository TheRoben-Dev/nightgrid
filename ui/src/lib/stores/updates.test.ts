import { describe, expect, it } from "vitest";
import { api, type UpdateStatus } from "../api";
import { settings } from "./settings.svelte";
import { updates } from "./updates.svelte";

const available: UpdateStatus = { kind: "available", version: "9.9.9", current: "0.4.2", notes: "x", date: null };

describe("updates store", () => {
  it("reports notConfigured outside a signed build", async () => {
    await updates.check(true);
    expect(updates.status?.kind).toBe("notConfigured");
    expect(updates.prompt).toBeNull();
  });

  it("prompts for a new version unless it was skipped", async () => {
    const orig = api.updateCheck;
    api.updateCheck = async () => available;
    try {
      await updates.check(false);
      expect(updates.prompt?.version).toBe("9.9.9");
      updates.skip();
      expect(updates.prompt).toBeNull();
      expect(settings.value.updateSkipVersion).toBe("9.9.9");
      await updates.check(false);
      expect(updates.prompt).toBeNull();
      await updates.check(true);
      expect(updates.prompt?.version).toBe("9.9.9");
      updates.later();
    } finally {
      api.updateCheck = orig;
    }
  });

  it("shows install errors and stays usable", async () => {
    updates.prompt = available.kind === "available" ? available : null;
    await updates.install();
    expect(updates.error).toMatch(/desktop app/);
    expect(updates.installing).toBe(false);
    expect(updates.phase).toBe("idle");
  });
});
