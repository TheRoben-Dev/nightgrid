// Component packs (feature store part 2): a store component's interface can
// come from a signed pack downloaded between app releases instead of the
// copy built into the app. The pack is verified and served by the app
// (`/components/<id>/<built>/…`); this decides whether to use it:
//
// - it must be newer than the interface the app is running,
// - built with the same Svelte, and
// - everything it needs from the app must be there unchanged: each host
//   module with each name it imports (host/manifest.json, written by the app
//   build) and each command it calls with the same hash.
//
// Anything else, or a pack that fails to load or render, falls back to the
// built-in copy, and a pack that failed is never tried again.

import type { Component } from "svelte";
import { SvelteMap } from "svelte/reactivity";
import { api, type ComponentPackInfo } from "./api";

declare const __UI_BUILT__: number | undefined;

export interface HostManifest {
  format: number;
  built: number;
  svelte: string;
  modules: Record<string, string[]>;
  commands: Record<string, string>;
}

export interface PackRequires {
  svelte: string;
  modules: Record<string, string[]>;
  commands: Record<string, string>;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type ViewModule = { default: Component<any> };
type Loader = () => Promise<ViewModule>;

const BAD_KEY = "nightgrid.badComponentPacks";

/** Why a pack can't run on this interface, or null when it can. */
export function packProblem(p: Pick<ComponentPackInfo, "built" | "requires">, host: HostManifest | null, bad: readonly string[] = [], id = ""): string | null {
  if (!host) return "this interface has no host modules";
  if (bad.includes(`${id}@${p.built}`)) return "this patch failed before";
  if (p.built <= host.built) return "the built-in copy is as new";
  let req: PackRequires;
  try {
    req = JSON.parse(p.requires) as PackRequires;
  } catch {
    return "unreadable requirements";
  }
  if (req.svelte !== host.svelte) return `built with Svelte ${req.svelte}, the app has ${host.svelte}`;
  for (const [mod, names] of Object.entries(req.modules ?? {})) {
    const have = host.modules[mod];
    if (!have) return `needs ${mod}, which this app doesn't share`;
    const missing = names.filter((n) => n !== "*" && !have.includes(n));
    if (missing.length) return `needs ${missing.join(", ")} from ${mod}`;
  }
  for (const [cmd, hash] of Object.entries(req.commands ?? {})) {
    if (host.commands[cmd] !== hash) return `the ${cmd} command changed`;
  }
  return null;
}

function readBad(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(BAD_KEY) ?? "[]") as unknown;
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string").slice(-50) : [];
  } catch {
    return [];
  }
}

class ComponentPacks {
  private host: Promise<HostManifest | null> | null = null;
  private packs: Promise<ComponentPackInfo[]> | null = null;
  /** Which component is running from a pack this session (id → built). */
  readonly running = new SvelteMap<string, number>();
  /** Why a stored pack isn't used (id → reason), for Settings → STORE. */
  readonly skipped = new SvelteMap<string, string>();

  hostManifest(): Promise<HostManifest | null> {
    this.host ??= fetch("/host/manifest.json")
      .then((r) => (r.ok ? (r.json() as Promise<HostManifest>) : null))
      .then((m) => (m && m.format === 1 && (m.built || 0) >= (typeof __UI_BUILT__ === "number" ? __UI_BUILT__ : 0) ? m : null))
      .catch(() => null);
    return this.host;
  }

  list(): Promise<ComponentPackInfo[]> {
    this.packs ??= api.componentPacks().catch(() => []);
    return this.packs;
  }

  /** Forget the cached list (after a download or removal). */
  refresh() {
    this.packs = null;
  }

  markBad(id: string, built: number, why: string) {
    const bad = [...readBad().filter((b) => b !== `${id}@${built}`), `${id}@${built}`];
    try {
      localStorage.setItem(BAD_KEY, JSON.stringify(bad));
    } catch {
      /* private mode: forgotten on restart, still skipped this run */
    }
    this.running.delete(id);
    this.skipped.set(id, `failed: ${why}`);
    console.warn(`component pack ${id}@${built} set aside: ${why}`);
  }

  /** The pack to use for a component, if any. */
  async usable(id: string): Promise<ComponentPackInfo | null> {
    const [host, packs] = await Promise.all([this.hostManifest(), this.list()]);
    const p = packs.find((x) => x.id === id);
    if (!p) return null;
    const why = packProblem(p, host, readBad(), id);
    if (why) {
      this.skipped.set(id, why);
      return null;
    }
    return p;
  }

  /** Loads a component from its pack, else the built-in copy. */
  async load(id: string, bundled: Loader): Promise<ViewModule> {
    const p = await this.usable(id).catch(() => null);
    if (p) {
      const base = `/components/${id}/${p.built}/`;
      try {
        if (p.style) addStyle(id, `${base}style.css`);
        const m = (await import(/* @vite-ignore */ `${base}${p.entry}`)) as ViewModule;
        if (typeof m.default !== "function") throw new Error("no component in the pack");
        this.running.set(id, p.built);
        this.skipped.delete(id);
        return m;
      } catch (e) {
        removeStyle(id);
        this.markBad(id, p.built, String((e as { message?: string })?.message ?? e));
      }
    }
    return bundled();
  }

  /** A view's renderer threw: set its pack aside. True when there was one (load again for the built-in copy). */
  failed(id: string, error: unknown): boolean {
    const built = this.running.get(id);
    if (built === undefined) return false;
    removeStyle(id);
    this.markBad(id, built, String((error as { message?: string })?.message ?? error));
    return true;
  }
}

function addStyle(id: string, href: string) {
  removeStyle(id);
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = href;
  link.dataset.componentPack = id;
  document.head.appendChild(link);
}

function removeStyle(id: string) {
  document.head.querySelectorAll(`link[data-component-pack="${id}"]`).forEach((l) => l.remove());
}

export const componentPacks = new ComponentPacks();

/** Loader references by component, so LazyView's cache stays stable. */
const loaders = new Map<string, Loader>();

/** A view loader that prefers the component's pack. */
export function packedView(id: string, bundled: Loader): Loader {
  let l = loaders.get(id);
  if (!l) {
    l = () => componentPacks.load(id, bundled);
    Object.assign(l, { componentId: id, bundled });
    loaders.set(id, l);
  }
  return l;
}

/** The component behind a loader made by packedView (for LazyView's fallback). */
export function packedInfo(l: unknown): { componentId: string; bundled: Loader } | null {
  const x = l as { componentId?: string; bundled?: Loader };
  return x && typeof x.componentId === "string" && x.bundled ? { componentId: x.componentId, bundled: x.bundled } : null;
}
