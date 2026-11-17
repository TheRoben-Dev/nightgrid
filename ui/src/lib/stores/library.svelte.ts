/**
 * Library state (M11): the open article, the search, and each guide's
 * checklist progress. Progress is a per-computer convenience kept in
 * localStorage; losing it loses nothing but ticks.
 */
import type { Shelf } from "../kb/catalog";
import { ui } from "./ui.svelte";

const KEY = "nightgrid.library.progress";

function read(): Record<string, Record<string, boolean>> {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return v && typeof v === "object" ? v : {};
  } catch {
    return {};
  }
}

class LibraryStore {
  /** The article being read, or null for the shelves. */
  current = $state<string | null>(null);
  query = $state("");
  shelf = $state<Shelf | "all">("all");
  progress = $state<Record<string, Record<string, boolean>>>(read());

  /** Articles read before this one, for BACK. */
  trail = $state<(string | null)[]>([]);

  /** Opens an article (from the palette, a "?" link or DRIFT). */
  open(id: string) {
    this.read(id);
    ui.go("library");
  }

  /** Moves to an article (or the shelves) inside the Library, remembering the way back. */
  read(id: string | null) {
    if (id === this.current) return;
    this.trail = [...this.trail.slice(-29), this.current];
    this.current = id;
  }

  back() {
    if (!this.trail.length) return;
    this.current = this.trail[this.trail.length - 1];
    this.trail = this.trail.slice(0, -1);
  }

  tick(id: string, key: string, on: boolean) {
    const cur = { ...(this.progress[id] ?? {}) };
    if (on) cur[key] = true;
    else delete cur[key];
    this.progress = { ...this.progress, [id]: cur };
    this.save();
  }

  resetProgress(id: string) {
    const next = { ...this.progress };
    delete next[id];
    this.progress = next;
    this.save();
  }

  private save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.progress));
    } catch {
      // Private window or blocked storage: ticks last for this session.
    }
  }
}

export const library = new LibraryStore();
