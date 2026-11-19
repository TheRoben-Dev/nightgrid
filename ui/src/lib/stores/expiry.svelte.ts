/** Expiry watch state (M10l): Services → EXPIRY WATCH. */
import { api, errorText, type ExpiryPref, type PrefTarget, type WatchInput, type WatchItem } from "../api";

class ExpiryStore {
  items = $state<WatchItem[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);
  busy = $state(false);
  /** Item being edited: 0 for a new one. */
  editing = $state<number | null>(null);
  /** Domain lookups running, by item id. */
  looking = $state<number[]>([]);
  /** The user's edits for rows Nightgrid finds by itself. */
  prefs = $state<ExpiryPref[]>([]);
  /** Automatic row being edited (a service or proxy certificate). */
  editingAuto = $state<PrefTarget | null>(null);

  async load() {
    try {
      [this.items, this.prefs] = await Promise.all([api.watchList(), api.expiryPrefs()]);
      this.loaded = true;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Saves; returns the item, or null with `error` set. */
  async save(input: WatchInput): Promise<WatchItem | null> {
    this.busy = true;
    try {
      const item = await api.watchSave(input);
      await this.load();
      return item;
    } catch (e) {
      this.error = errorText(e);
      return null;
    } finally {
      this.busy = false;
    }
  }

  /** Saves the edits for an automatic row; true when saved. */
  async savePref(pref: ExpiryPref): Promise<boolean> {
    this.busy = true;
    try {
      this.prefs = await api.expiryPrefSave(pref);
      this.error = null;
      return true;
    } catch (e) {
      this.error = errorText(e);
      return false;
    } finally {
      this.busy = false;
    }
  }

  async remove(id: number) {
    try {
      await api.watchDelete(id);
      await this.load();
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Re-reads every certificate file. */
  async check() {
    this.busy = true;
    try {
      this.items = await api.watchCheck();
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.busy = false;
    }
  }

  async lookup(id: number, via: number) {
    this.looking = [...this.looking, id];
    try {
      await api.watchLookup(id, via);
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.looking = this.looking.filter((x) => x !== id);
      await this.load();
    }
  }
}

export const expiry = new ExpiryStore();
