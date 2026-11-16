import { api, errorText, type Alert, type Note, type NoteInput } from "../api";
import { KIND_LABEL } from "./alerts.svelte";
import { redact } from "../helper/prompt";

/** Which notes the list shows by host. */
export type HostFilter = "all" | "none" | number;

/**
 * Whether a runbook note applies to an alert (same rule as
 * `notes::runbook_matches` in Rust): marked as a runbook, scoped to
 * something, and every part of the scope fits.
 */
export function runbookMatches(n: Pick<Note, "runbook" | "alertKinds" | "hostId" | "serviceId">, a: Alert): boolean {
  if (!n.runbook || (!n.alertKinds.length && n.hostId === null && n.serviceId === null)) return false;
  const kind = !n.alertKinds.length || n.alertKinds.includes(a.kind);
  const host = n.hostId === null || n.hostId === a.hostId;
  const svc = n.serviceId === null || new RegExp(`^svc:${n.serviceId}(:|$)`).test(a.key);
  return kind && host && svc;
}

/** "Disk full or Filling up, on nas" for the editor and reader. */
export function runbookScope(
  n: Pick<Note, "alertKinds" | "hostId" | "serviceId">,
  hostName: (id: number | null) => string | null,
  serviceName: (id: number | null) => string | null,
): string {
  const parts = [n.alertKinds.length ? n.alertKinds.map((k) => KIND_LABEL[k].toLowerCase()).join(" or ") : "any alert"];
  if (n.hostId !== null) parts.push(`on ${hostName(n.hostId)}`);
  if (n.serviceId !== null) parts.push(`about ${serviceName(n.serviceId)}`);
  return parts.join(", ");
}

export const emptyInput = (hostId: number | null = null): NoteInput => ({ title: "", body: "", hostId, tags: [], pinned: false, hideFromDrift: false, runbook: false, alertKinds: [], serviceId: null });

/** Splits "a, #b c" into tags. */
export const parseTags = (s: string): string[] =>
  s
    .split(/[\s,]+/)
    .map((t) => t.replace(/^#/, "").trim().toLowerCase())
    .filter((t, i, all) => t && all.indexOf(t) === i);

/** Whether saving would be refused for holding a secret (the real check is in Rust). */
export const hasSecret = (i: NoteInput): boolean => redact(i.title) !== i.title || redact(i.body) !== i.body;

export function matches(n: Note, q: string, tag: string | null, host: HostFilter): boolean {
  if (tag && !n.tags.includes(tag)) return false;
  if (host === "none" ? n.hostId !== null : host !== "all" && n.hostId !== host) return false;
  if (!q) return true;
  const hay = `${n.title}\n${n.body}\n${n.tags.join(" ")}`.toLowerCase();
  return q
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => hay.includes(w));
}

class NotesStore {
  notes = $state<Note[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);
  selected = $state<number | null>(null);
  /** Note being edited (`id` null = new), or null when reading. */
  draft = $state<{ id: number | null; input: NoteInput } | null>(null);
  saving = $state(false);
  saveError = $state<string | null>(null);
  query = $state("");
  tag = $state<string | null>(null);
  host = $state<HostFilter>("all");

  readonly visible = $derived(this.notes.filter((n) => matches(n, this.query.trim(), this.tag, this.host)));
  readonly tags = $derived(
    this.notes
      .flatMap((n) => n.tags)
      .filter((t, i, all) => all.indexOf(t) === i)
      .sort(),
  );
  readonly current = $derived(this.notes.find((n) => n.id === this.selected) ?? null);

  async load() {
    try {
      this.notes = await api.notesList();
      this.error = null;
      if (this.selected === null || !this.notes.some((n) => n.id === this.selected)) this.selected = this.notes[0]?.id ?? null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loaded = true;
    }
  }

  startNew(hostId: number | null = typeof this.host === "number" ? this.host : null) {
    this.draft = { id: null, input: emptyInput(hostId) };
    this.saveError = null;
  }

  edit(n: Note) {
    this.draft = { id: n.id, input: { title: n.title, body: n.body, hostId: n.hostId, tags: [...n.tags], pinned: n.pinned, hideFromDrift: n.hideFromDrift, runbook: n.runbook, alertKinds: [...n.alertKinds], serviceId: n.serviceId } };
    this.saveError = null;
  }

  cancel() {
    this.draft = null;
    this.saveError = null;
  }

  /** Replaces secret-looking values in the draft with ••••. */
  maskDraft() {
    if (!this.draft) return;
    const i = this.draft.input;
    this.draft.input = { ...i, title: redact(i.title), body: redact(i.body) };
  }

  async save(): Promise<boolean> {
    if (!this.draft) return false;
    this.saving = true;
    try {
      const n = await api.noteSave(this.draft.id, this.draft.input);
      this.draft = null;
      this.saveError = null;
      await this.load();
      this.selected = n.id;
      return true;
    } catch (e) {
      this.saveError = errorText(e);
      return false;
    } finally {
      this.saving = false;
    }
  }

  async togglePin(n: Note) {
    try {
      await api.noteSave(n.id, { title: n.title, body: n.body, hostId: n.hostId, tags: n.tags, pinned: !n.pinned, hideFromDrift: n.hideFromDrift, runbook: n.runbook, alertKinds: n.alertKinds, serviceId: n.serviceId });
      await this.load();
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async remove(id: number) {
    try {
      await api.noteDelete(id);
      if (this.selected === id) this.selected = null;
      await this.load();
    } catch (e) {
      this.error = errorText(e);
    }
  }
}

export const notes = new NotesStore();
