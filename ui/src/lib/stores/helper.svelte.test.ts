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
    await settings.update({ helperEnabled: false, helperModel: "qwen3-4b-instruct-2507-q4km" });
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

  it("recommends by memory, keeps the old model until the new one is ready, deletes unused ones", async () => {
    mockHelper.reset(true);
    await helper.setEnabled(true);
    await helper.loadModels();
    const m = helper.models!;
    expect(m.recommended).toBe("qwen3-8b-q4km");
    expect(m.models.map((x) => x.fit)).toEqual(["good", "good", "no"]);
    const p = helper.download("qwen3-8b-q4km", true);
    expect(settings.value.helperModel).toBe("qwen3-4b-instruct-2507-q4km");
    expect(helper.status?.state).toBe("ready");
    await p;
    expect(settings.value.helperModel).toBe("qwen3-8b-q4km");
    expect(helper.status?.id).toBe("qwen3-8b-q4km");
    expect(helper.models?.active).toBe("qwen3-8b-q4km");
    await helper.deleteModel("qwen3-4b-instruct-2507-q4km");
    expect(helper.models?.models[0].state).toBe("missing");
    expect(helper.ready).toBe(true);
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
    expect(helper.messages[1].error).toMatch(/Download Qwen3 4B/);
    expect(helper.drift).toBe("error");
  });

  it("explain attaches labelled text and asks once the model is ready", async () => {
    await helper.setEnabled(true);
    helper.draft = "";
    await helper.explain({ source: "Docker logs: web", text: "error: boom", question: "What's wrong?", host: "nas" });
    // No model yet: the panel opens with the text attached and the question waiting.
    expect(helper.open).toBe(true);
    expect(helper.messages).toEqual([]);
    expect(helper.draft).toBe("What's wrong?");
    expect(helper.context).toMatchObject({ host: "nas", selection: "error: boom", source: "Docker logs: web" });

    mockHelper.reset(true);
    await helper.explain({ source: "Docker logs: web", text: "error: boom\nTOKEN=abc123", question: "What's wrong?" });
    const prompt = helper.messages[1].sent?.at(-1)?.content ?? "";
    expect(prompt).toContain("Attached (Docker logs: web):");
    expect(prompt).toContain("error: boom");
    expect(prompt).not.toContain("abc123");
    expect(helper.messages[0].attached).toBe("Docker logs: web · 2 lines");
    // Used once.
    expect(helper.context.selection).toBeNull();
    expect(helper.context.source).toBeNull();
  });

  it("turning off closes the panel", async () => {
    await helper.setEnabled(true);
    helper.toggle(true);
    await helper.setEnabled(false);
    expect(helper.open).toBe(false);
  });
});
