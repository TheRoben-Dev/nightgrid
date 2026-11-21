import { cmpVersion, loadBundled, whatsNew, type Release } from "../releases";
import { settings } from "./settings.svelte";

/** Wait after start-up before "What's new", so it doesn't land on a loading screen. */
const SHOW_DELAY_MS = 2500;

/** Release notes (D-204): "What's new" after an update and the release log. */
class ReleasesStore {
  list = $state<Release[]>([]);
  loaded = $state(false);
  /** Releases shown in the "What's new" dialog (empty = closed). */
  shown = $state<Release[]>([]);
  /** Version this app runs, once known. */
  current = $state<string | null>(null);
  private started = false;

  async load(): Promise<Release[]> {
    if (!this.loaded) {
      try {
        this.list = await loadBundled();
      } catch (e) {
        console.error("release notes failed to load", e);
      }
      this.loaded = true;
    }
    return this.list;
  }

  /** This version's release, from the bundled notes. */
  get running(): Release | null {
    return this.list.find((r) => r.version === this.current) ?? null;
  }

  /** Once per run, after settings and the app version are known. */
  async start(version: string) {
    if (this.started) return;
    this.started = true;
    this.current = version;
    const list = await this.load();
    const last = settings.value.lastSeenVersion ?? null;
    // A first start with this feature on a computer that ran Nightgrid before
    // shows this release; tourOffered is set once the app has been opened.
    const show = whatsNew(list, version, last, settings.value.tourOffered);
    if (!show.length) {
      if (!last || cmpVersion(version, last) !== 0) void settings.update({ lastSeenVersion: version });
      return;
    }
    setTimeout(() => (this.shown = show), SHOW_DELAY_MS);
  }

  /** Close "What's new" and remember this version. */
  dismiss() {
    this.shown = [];
    if (this.current && settings.value.lastSeenVersion !== this.current) void settings.update({ lastSeenVersion: this.current });
  }

  /** Open "What's new" for one release (from the release log). */
  async replay(version?: string) {
    const list = await this.load();
    const r = list.find((x) => x.version === (version ?? this.current)) ?? list[0];
    if (r) this.shown = [r];
  }
}

export const releases = new ReleasesStore();
