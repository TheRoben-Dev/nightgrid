import { describe, expect, it } from "vitest";
import { confirmLine, images, initialValues, portsOf, runbookAdded, runbookInput, startersFor, toValues, valuesProblem } from "./starters";
import { mockStarters } from "./mockStarters";
import type { Note } from "./api";

describe("starters", async () => {
  const cat = await mockStarters.list();
  const media = cat.blueprints[0];
  const dns = cat.blueprints[1];

  it("starts the form from defaults", () => {
    expect(initialValues(media)).toEqual({ "media/TZ": "Etc/UTC", "media/MEDIA_DIR": "/srv/media" });
    expect(initialValues(dns)).toEqual({ "dns/PIHOLE_PASSWORD": "" });
  });

  it("says what still needs filling in", () => {
    const v = initialValues(media);
    expect(valuesProblem(media, v, "/opt/stacks")).toBeNull();
    expect(valuesProblem(media, v, "stacks")).toMatch(/absolute folder/);
    expect(valuesProblem(media, v, "/opt/$HOME")).toMatch(/absolute folder/);
    expect(valuesProblem(media, { ...v, "media/MEDIA_DIR": "srv" }, "/opt/stacks")).toMatch(/Folder holding your media/);
    expect(valuesProblem(media, { ...v, "media/TZ": "a\nb" }, "/opt/stacks")).toMatch(/one line/);
    // A generated secret may stay empty.
    expect(valuesProblem(dns, initialValues(dns), "/opt/stacks")).toBeNull();
    const needs = { ...media, stacks: [{ ...media.stacks[0], env: [{ ...media.stacks[0].env[0], default: "" }] }] };
    expect(valuesProblem(needs, { "media/TZ": " " }, "/opt/stacks")).toBe("Fill in “Time zone”.");
  });

  it("sends only typed values, trimmed", () => {
    expect(toValues({ "a/X": " 1 ", "a/Y": "", "a/Z": "  " })).toEqual([{ key: "a/X", value: "1" }]);
  });

  it("describes ports and images", () => {
    expect(portsOf(media)).toEqual({ lan: ["Jellyfin 8096/tcp"], local: ["Sonarr 8989/tcp"] });
    expect(images("services:\n  a:\n    image: jellyfin/jellyfin:latest\n  b:\n    image: 'caddy:2' # web\n")).toEqual(["jellyfin/jellyfin:latest", "caddy:2"]);
    expect(confirmLine(1, "server01")).toMatch(/^Apply 1 step to server01\?/);
    expect(confirmLine(3, "server01")).toMatch(/^Apply 3 steps/);
  });

  it("turns runbooks into runbook notes and knows which are added", () => {
    const disk = cat.runbooks[0];
    const i = runbookInput(disk);
    expect(i.runbook).toBe(true);
    expect(i.alertKinds).toEqual(["diskFull", "diskFilling"]);
    expect(i.hostId).toBeNull();
    const note = { id: 1, title: disk.title, body: "", hostId: null, tags: [], pinned: false, hideFromDrift: false, runbook: true, alertKinds: [], serviceId: null, updatedAt: 0 } as Note;
    expect(runbookAdded([note], disk)).toBe(note);
    expect(startersFor("diskFull", cat.runbooks, [])).toEqual([disk]);
    expect(startersFor("diskFull", cat.runbooks, [note])).toEqual([]);
    expect(startersFor("hostOffline", cat.runbooks, [])).toEqual([]);
  });
});
