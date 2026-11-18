/** State of the 1.1 Fleet tabs (CATALOG, DRILLS, CAPACITY, REPLAY), kept across tab switches. */
import { replayMarkdown } from "../fleet11";
import { api, errorText, type CatalogHost, type Drill, type DrillBoard, type DrillRun, type HostCapacity, type Incident, type Replay } from "../api";

function stamp() {
  return new Date().toISOString().slice(0, 10);
}

/** Saves Markdown where the user picks; returns the path or null. */
export async function saveReport(name: string, md: string): Promise<string | null> {
  return api.reportSave(name, md);
}

class CatalogStore {
  hosts = $state<CatalogHost[]>([]);
  building = $state(false);
  builtAt = $state<number | null>(null);
  error = $state<string | null>(null);
  saved = $state<string | null>(null);

  async build(ids: number[]) {
    if (this.building || !ids.length) return;
    this.building = true;
    this.error = null;
    try {
      this.hosts = await api.catalogBuild(ids);
      this.builtAt = Date.now();
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.building = false;
    }
  }

  async export() {
    try {
      this.saved = await saveReport(`nightgrid-catalog-${stamp()}.md`, await api.catalogMarkdown(this.hosts));
    } catch (e) {
      this.error = errorText(e);
    }
  }
}

class DrillStore {
  board = $state<DrillBoard>({ drills: [], running: [] });
  loaded = $state(false);
  error = $state<string | null>(null);
  open = $state<number | null>(null);
  runs = $state<DrillRun[]>([]);
  editing = $state<Drill | null>(null);
  /** Started from this window (the board's list covers scheduled ones). */
  starting = $state<number[]>([]);

  async load() {
    try {
      this.board = await api.drillsList();
      this.loaded = true;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async select(id: number | null) {
    this.open = id;
    this.runs = id == null ? [] : await api.drillRuns(id).catch(() => []);
  }

  async save(d: Drill) {
    try {
      const id = await api.drillSave(d);
      this.editing = null;
      await this.load();
      await this.select(id);
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async remove(id: number) {
    try {
      await api.drillDelete(id);
      if (this.open === id) await this.select(null);
      await this.load();
    } catch (e) {
      this.error = errorText(e);
    }
  }

  isRunning = (id: number) => this.starting.includes(id) || this.board.running.includes(id);

  async run(id: number) {
    if (this.isRunning(id)) return;
    this.starting = [...this.starting, id];
    this.error = null;
    try {
      await api.drillRun(id);
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.starting = this.starting.filter((x) => x !== id);
      await this.load();
      if (this.open === id) await this.select(id);
    }
  }

  async export(id: number, name: string) {
    try {
      const safe = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "drill";
      await saveReport(`nightgrid-drill-${safe}-${stamp()}.md`, await api.drillReport(id));
    } catch (e) {
      this.error = errorText(e);
    }
  }
}

class CapacityStore {
  list = $state<HostCapacity[]>([]);
  days = $state(30);
  loaded = $state(false);
  error = $state<string | null>(null);

  async load(days = this.days) {
    this.days = days;
    try {
      this.list = await api.capacityList(days);
      this.loaded = true;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }
}

class ReplayStore {
  incidents = $state<Incident[]>([]);
  days = $state(14);
  loaded = $state(false);
  error = $state<string | null>(null);
  replay = $state<Replay | null>(null);
  /** Scrubber position, unix seconds. */
  at = $state(0);
  playing = $state(false);

  async load(days = this.days) {
    this.days = days;
    try {
      this.incidents = await api.replayIncidents(days);
      this.loaded = true;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async open(i: Incident) {
    this.playing = false;
    try {
      this.replay = await api.replayLoad(i);
      this.at = this.replay.from;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async export() {
    if (!this.replay) return;
    try {
      await saveReport(`nightgrid-incident-${stamp()}.md`, replayMarkdown(this.replay));
    } catch (e) {
      this.error = errorText(e);
    }
  }
}

export const catalog = new CatalogStore();
export const drills = new DrillStore();
export const capacity = new CapacityStore();
export const replay = new ReplayStore();
