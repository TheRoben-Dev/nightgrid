import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { api, type Host, type PlatformSupport } from "../api";
import { hosts } from "./hosts.svelte";

/** Feature ids of `nightgrid_core::win::support` (the tabs and views that run things on a host). */
export type FeatureId = string;

/**
 * What works on Windows hosts (OpenSSH for Windows). The table comes from
 * the backend so the words match the errors its commands give. A host's OS
 * is learned the first time it connects (`hosts://os`).
 */
class PlatformStore {
  windows = $state<PlatformSupport[]>([]);
  private started = false;

  start() {
    if (this.started) return;
    this.started = true;
    api
      .platformWindowsSupport()
      .then((t) => (this.windows = t))
      .catch(() => {
        /* without the table, views simply show their own errors */
      });
    if (isTauri()) {
      void listen<{ hostId: number; os: string }>("hosts://os", (e) => {
        hosts.hosts = hosts.hosts.map((h) => (h.id === e.payload.hostId ? { ...h, os: e.payload.os } : h));
      });
    }
  }

  isWindows(host: Pick<Host, "os"> | null | undefined): boolean {
    return host?.os === "windows";
  }

  /** The Windows note for `feature` on `host`, or undefined when it works as usual there. */
  note(host: Pick<Host, "os"> | null | undefined, feature: FeatureId): PlatformSupport | undefined {
    if (!this.isWindows(host)) return undefined;
    return this.windows.find((s) => s.feature === feature);
  }

  /** The note when `feature` can't run on `host` (planned or impossible there). */
  blocked(host: Pick<Host, "os"> | null | undefined, feature: FeatureId): PlatformSupport | undefined {
    const n = this.note(host, feature);
    return n && (n.level === "planned" || n.level === "none") ? n : undefined;
  }
}

export const platform = new PlatformStore();
