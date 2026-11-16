/**
 * Custom command widgets (M10x): definitions from the database, the latest
 * reading of each, and refresh timers that run only while a tile is shown.
 */
import { api, errorText, type CmdReading, type CmdWidget } from "../api";

class CmdWidgetsStore {
  list = $state<CmdWidget[]>([]);
  loaded = $state(false);
  readings = $state<Record<number, CmdReading>>({});
  errors = $state<Record<number, string>>({});
  running = $state<Record<number, boolean>>({});
  /** The widget being edited (id 0 = new), and the dashboard slot to fill on save. */
  editing = $state<CmdWidget | null>(null);
  private watchers = new Map<number, { count: number; timer: ReturnType<typeof setInterval> | null }>();
  private loading: Promise<void> | null = null;

  byId = (id: number): CmdWidget | undefined => this.list.find((w) => w.id === id);

  load(): Promise<void> {
    this.loading ??= (async () => {
      try {
        this.list = await api.cmdWidgetsList();
      } catch (e) {
        console.warn("command widgets", errorText(e));
      } finally {
        this.loaded = true;
        this.loading = null;
      }
    })();
    return this.loading;
  }

  async run(id: number) {
    if (this.running[id]) return;
    this.running = { ...this.running, [id]: true };
    try {
      const r = await api.cmdWidgetRun(id);
      this.readings = { ...this.readings, [id]: r };
      const { [id]: _, ...rest } = this.errors;
      this.errors = rest;
    } catch (e) {
      this.errors = { ...this.errors, [id]: errorText(e) };
    } finally {
      this.running = { ...this.running, [id]: false };
    }
  }

  /** Starts refreshing `id` while at least one tile shows it; returns the stop function. */
  watch(id: number): () => void {
    const cur = this.watchers.get(id);
    if (cur) cur.count++;
    else {
      const entry: { count: number; timer: ReturnType<typeof setInterval> | null } = { count: 1, timer: null };
      this.watchers.set(id, entry);
      void (async () => {
        if (!this.loaded) await this.load();
        const w = this.byId(id);
        if (!w || !this.watchers.has(id)) return;
        if (!this.readings[id]) void this.run(id);
        entry.timer = setInterval(() => {
          if (typeof document === "undefined" || !document.hidden) void this.run(id);
        }, w.intervalSecs * 1000);
      })();
    }
    return () => {
      const e = this.watchers.get(id);
      if (!e) return;
      if (--e.count <= 0) {
        if (e.timer) clearInterval(e.timer);
        this.watchers.delete(id);
      }
    };
  }

  /** Restarts a timer after its interval changed. */
  private rewatch(id: number) {
    const e = this.watchers.get(id);
    if (!e) return;
    if (e.timer) clearInterval(e.timer);
    const w = this.byId(id);
    e.timer = w ? setInterval(() => (typeof document === "undefined" || !document.hidden) && void this.run(id), w.intervalSecs * 1000) : null;
  }

  async save(w: CmdWidget): Promise<CmdWidget> {
    const saved = await api.cmdWidgetSave(w);
    this.list = [...this.list.filter((x) => x.id !== saved.id), saved].sort((a, b) => a.name.localeCompare(b.name));
    this.rewatch(saved.id);
    void this.run(saved.id);
    return saved;
  }

  async remove(id: number) {
    await api.cmdWidgetDelete(id);
    this.list = this.list.filter((w) => w.id !== id);
    const { [id]: _, ...rest } = this.readings;
    this.readings = rest;
  }
}

export const cmdWidgets = new CmdWidgetsStore();
