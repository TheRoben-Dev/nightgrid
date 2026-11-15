import { beforeEach, describe, expect, it } from "vitest";
import { mockHelper } from "../mockHelper";
import { helper } from "./helper.svelte";
import { settings } from "./settings.svelte";
import { ui } from "./ui.svelte";

describe("helper store (browser mock)", () => {
  beforeEach(async () => {
    mockHelper.reset(false);
    helper.messages = [];
    helper.clearSpotlight();
    await settings.update({ helperEnabled: false });
    ui.go("overview");
  });

  it("is asleep until turned on, then downloads the model", async () => {
    expect(helper.drift).toBe("sleep");
    await helper.setEnabled(true);
    expect(helper.drift).toBe("idle");
    expect(helper.status?.state).toBe("missing");
    await helper.download();
    expect(helper.ready).toBe(true);
    expect(helper.progress).toBeNull();
  });

  it("streams a reply, keeps the prompt and runs show-me actions", async () => {
    mockHelper.reset(true);
    await helper.setEnabled(true);
    helper.toggle(true);
    await helper.ask("How do I export a backup?");
    const [q, a] = helper.messages;
    expect(q.text).toBe("How do I export a backup?");
    expect(a.streaming).toBe(false);
    expect(a.text).toContain("Backups live in Settings");
    expect(a.text).not.toContain("[[");
    expect(a.actions).toEqual([
      { kind: "show", view: "settings" },
      { kind: "point", id: "settings-backup", note: "Export and import are here" },
    ]);
    expect(a.sent?.[0].role).toBe("system");
    expect(a.sources?.length).toBeGreaterThan(0);

    // The spotlight needs the control in the page.
    const el = document.createElement("section");
    el.dataset.guide = "settings-backup";
    el.scrollIntoView = () => {};
    document.body.append(el);
    await helper.run(a.actions);
    expect(ui.section).toBe("settings");
    expect(helper.spotlight?.id).toBe("settings-backup");
    expect(helper.drift).toBe("point");
    helper.clearSpotlight();
    el.remove();
  });

  it("reports errors without losing the conversation", async () => {
    await helper.setEnabled(true);
    await helper.ask("hello");
    expect(helper.messages[1].error).toMatch(/Download the model/);
    expect(helper.drift).toBe("error");
  });

  it("turning off closes the panel", async () => {
    await helper.setEnabled(true);
    helper.toggle(true);
    await helper.setEnabled(false);
    expect(helper.open).toBe(false);
  });
});
