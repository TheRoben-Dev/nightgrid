/**
 * Library content updates (D-174). CI publishes the docs/kb shelves as a
 * signed pack whenever only Library content changes; the backend fetches and
 * verifies it, and this store swaps it into the Library (articles, figures,
 * catalog) when it's newer than the copy bundled with the app. Everything
 * works without it: offline, turned off or nothing newer means the bundled
 * Library, exactly as before.
 */
import { api, errorText, type KbPackStatus } from "../api";
import { applyCatalog } from "../kb/catalog";
import { BUNDLED_KB } from "../kb/version";

export { kbVersionLabel } from "../kb/version";
import { settings } from "./settings.svelte";

/** Wait after launch, then every few hours while the app stays open. */
const LAUNCH_DELAY_MS = 20_000;
const EVERY_MS = 6 * 60 * 60 * 1000;

class KbContentStore {
  status = $state<KbPackStatus | null>(null);
  checking = $state(false);
  error = $state<string | null>(null);
  /** Bumped when the Library's text changes; views re-read articles on it. */
  rev = $state(0);
  /** The content version in use and where it came from. */
  version = $state(BUNDLED_KB);
  source = $state<"bundled" | "update">("bundled");
  /** The pack version applied to the article text (0 = bundled). */
  private applied = 0;
  /** Something read the article text, so updates are swapped in right away. */
  private loaded = false;
  private pending: Promise<void> | null = null;
  private timer: ReturnType<typeof setInterval> | null = null;

  /** A stored pack worth using: verified by the backend and newer than the bundled copy. */
  private usable(): number {
    const v = this.status?.installed?.version ?? 0;
    return v > BUNDLED_KB ? v : 0;
  }

  /** On launch: use a stored pack's catalog (palette, topics), then check quietly when enabled. */
  async start() {
    try {
      this.status = await api.kbPackStatus();
      await this.useCatalog();
    } catch (e) {
      console.warn("Library content status failed", e);
    }
    setTimeout(() => void this.check(false), LAUNCH_DELAY_MS);
    this.timer ??= setInterval(() => void this.check(false), EVERY_MS);
  }

  private async useCatalog() {
    const v = this.usable();
    if (!v) {
      applyCatalog(null);
      this.version = BUNDLED_KB;
      this.source = "bundled";
      return;
    }
    const pack = await api.kbPackLoad(false);
    if (!pack || pack.version !== v) return;
    applyCatalog(pack.catalog);
    this.version = v;
    this.source = "update";
  }

  /**
   * Brings the article text up to the newest usable content. Called before
   * the Library or DRIFT read articles; cheap when nothing changed.
   */
  ensure(): Promise<void> {
    this.loaded = true;
    if (!this.pending) this.pending = this.apply().finally(() => (this.pending = null));
    return this.pending;
  }

  private async apply(): Promise<void> {
    const want = this.usable();
    if (want === this.applied) return;
    const [lib, figs] = await Promise.all([import("../kb/library"), import("../kb/figures")]);
    if (want) {
      const pack = await api.kbPackLoad(true);
      if (!pack || pack.version !== want) return;
      lib.applyPack(pack.files);
      figs.applyFigures(pack.files);
      applyCatalog(pack.catalog);
    } else {
      lib.applyPack(null);
      figs.applyFigures(null);
      applyCatalog(null);
    }
    this.applied = want;
    this.version = want || BUNDLED_KB;
    this.source = want ? "update" : "bundled";
    this.rev++;
  }

  /** `manual` checks even when updates are off and shows errors. */
  async check(manual: boolean) {
    if (this.checking || (!manual && !settings.value.kbUpdates)) return;
    this.checking = true;
    if (manual) this.error = null;
    try {
      const s = await api.kbPackCheck(BUNDLED_KB);
      this.status = s;
      if (manual) this.error = s.error;
      if (s.updated) {
        await this.useCatalog();
        // Only swap text that's already loaded; otherwise the next read picks it up.
        if (this.loaded) await this.ensure();
      }
    } catch (e) {
      if (manual) this.error = errorText(e);
    } finally {
      this.checking = false;
    }
  }

  /** Deletes the downloaded content and goes back to the bundled Library. */
  async useBundled() {
    try {
      this.status = await api.kbPackRemove();
      await this.useCatalog();
      await this.ensure();
    } catch (e) {
      this.error = errorText(e);
    }
  }
}

export const kbContent = new KbContentStore();
