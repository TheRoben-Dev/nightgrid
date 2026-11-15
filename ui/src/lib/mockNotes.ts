/**
 * Browser-only stand-in for notes: in memory, with a sample runbook, for
 * `pnpm dev`, tests and screenshots. The real secret check is in Rust
 * (core/src/redact.rs); here anything `redact()` would change is refused.
 */
import type { NoteInput_Deserialize as NoteInput, Note_Serialize as Note } from "./bindings";
import { redact } from "./helper/prompt";

let nextId = 1;
let notes: Note[] = [];

const now = () => Math.floor(Date.now() / 1000);

function check(i: NoteInput): Note {
  const title = (i.title ?? "").trim();
  if (!title) throw new Error("invalid value for title: must be 1-120 characters");
  const body = (i.body ?? "").trimEnd();
  for (const [field, text] of [
    ["title", title],
    ["body", body],
  ] as const) {
    if (redact(text) !== text)
      throw new Error(`invalid value for ${field}: looks like it contains a password or token. Mask it (or keep it in a password manager) before saving`);
  }
  const tags = [...new Set((i.tags ?? []).map((t) => t.trim().replace(/^#/, "").toLowerCase()).filter(Boolean))];
  return { id: 0, title, body, hostId: i.hostId ?? null, tags, pinned: !!i.pinned, updatedAt: now() };
}

const sorted = () => [...notes].sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt || b.id - a.id);

export function resetMockNotes(seed = true) {
  nextId = 1;
  notes = [];
  if (!seed) return;
  notes.push({
    id: nextId++,
    title: "Restart the media stack",
    body: [
      "When streams stall or the web UI hangs.",
      "",
      "1. Check what's running:",
      "```sh",
      "docker compose -f /opt/stacks/media/compose.yaml ps",
      "```",
      "2. Restart it (takes about 20 s):",
      "```sh",
      "docker compose -f /opt/stacks/media/compose.yaml restart",
      "```",
      "3. If a container keeps restarting, read its logs in **Docker → Logs**.",
    ].join("\n"),
    hostId: 1,
    tags: ["docker", "runbook"],
    pinned: true,
    updatedAt: now() - 3600,
  });
  notes.push({
    id: nextId++,
    title: "Disk layout",
    body: "- `/` on the NVMe\n- `/srv` on the mirror (ZFS pool `tank`)\n\nScrub runs monthly from a systemd timer.",
    hostId: null,
    tags: ["storage"],
    pinned: false,
    updatedAt: now() - 86400 * 3,
  });
}
resetMockNotes();

export const mockNotes = {
  list: async (): Promise<Note[]> => sorted(),
  save: async (id: number | null, i: NoteInput): Promise<Note> => {
    const n = check(i);
    if (id === null) {
      n.id = nextId++;
      notes.push(n);
    } else {
      if (!notes.some((x) => x.id === id)) throw new Error("note not found");
      n.id = id;
      notes = notes.map((x) => (x.id === id ? n : x));
    }
    return n;
  },
  delete: async (id: number): Promise<null> => {
    notes = notes.filter((n) => n.id !== id);
    return null;
  },
};
