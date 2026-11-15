import { beforeEach, describe, expect, it } from "vitest";
import { parseMd } from "../helper/md";
import { resetMockNotes } from "../mockNotes";
import { hasSecret, matches, notes, parseTags } from "./notes.svelte";

describe("notes", () => {
  beforeEach(() => {
    resetMockNotes();
    notes.draft = null;
    notes.selected = null;
  });

  it("parses tags and filters", () => {
    expect(parseTags("Docker, #backup docker  ")).toEqual(["docker", "backup"]);
    const n = { id: 1, title: "Restart media", body: "docker compose restart", hostId: 2, tags: ["docker"], pinned: false, hideFromDrift: false, updatedAt: 0 };
    expect(matches(n, "media restart", null, "all")).toBe(true);
    expect(matches(n, "", "backup", "all")).toBe(false);
    expect(matches(n, "", null, "none")).toBe(false);
    expect(matches(n, "", null, 2)).toBe(true);
  });

  it("creates, masks secrets and deletes", async () => {
    await notes.load();
    expect(notes.notes[0].pinned).toBe(true);
    notes.startNew(null);
    notes.draft!.input.title = "DB";
    notes.draft!.input.body = "DB_PASSWORD=hunter22";
    expect(hasSecret(notes.draft!.input)).toBe(true);
    expect(await notes.save()).toBe(false);
    expect(notes.saveError).toMatch(/password or token/);
    notes.maskDraft();
    expect(notes.draft!.input.body).toBe("DB_PASSWORD=••••");
    expect(await notes.save()).toBe(true);
    const id = notes.selected!;
    expect(notes.current?.title).toBe("DB");
    await notes.remove(id);
    expect(notes.notes.some((n) => n.id === id)).toBe(false);
  });

  it("renders headings only when asked", () => {
    expect(parseMd("# Title\ntext", { headings: true }).map((b) => b.kind)).toEqual(["h", "p"]);
    expect(parseMd("# Title\ntext").map((b) => b.kind)).toEqual(["p"]);
    const b = parseMd("3. three\n4. four");
    expect(b[0]).toMatchObject({ kind: "ol", start: 3 });
  });
});
