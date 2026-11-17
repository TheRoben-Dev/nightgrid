/**
 * Web UIs in their own Nightgrid window (D-147): opening one, and what to
 * show when it can't open there (a dead port, or an HTTPS certificate the
 * window can't step past), with the browser as the way out.
 */
import { api, errorText, type WebUiTarget } from "../api";
import { openExternal } from "../clipboard";
import { tunnels } from "./tunnels.svelte";

export interface WebUiNotice {
  title: string;
  hostId: number | null;
  /** The address as given (as the host sees it, when there is a host). */
  url: string;
  text: string;
  /** Can still be opened in the browser (an untrusted certificate). */
  browser: boolean;
}

/** The URL of a container's web UI on `host` (mirrors core `webui::url_for`). */
export function targetUrl(t: WebUiTarget, host: string): string {
  let h = t.bind ?? host;
  if (h.includes(":") && !h.startsWith("[")) h = `[${h}]`;
  const dflt = (t.scheme === "http" && t.port === 80) || (t.scheme === "https" && t.port === 443);
  return `${t.scheme}://${h}${dflt ? "" : `:${t.port}`}${t.path}`;
}

/** A short label for one target, e.g. "8080" or "https 9443 /admin". */
export function targetLabel(t: WebUiTarget): string {
  return [t.scheme === "https" ? "https" : "", String(t.port), t.path !== "/" ? t.path : ""].filter(Boolean).join(" ");
}

class WebUiStore {
  /** Keys (`hostId|url`) being opened. */
  busy = $state<string[]>([]);
  notice = $state<WebUiNotice | null>(null);

  key = (hostId: number | null, url: string) => `${hostId ?? ""}|${url}`;
  isBusy = (hostId: number | null, url: string) => this.busy.includes(this.key(hostId, url));

  async open(hostId: number | null, url: string, title: string): Promise<boolean> {
    const k = this.key(hostId, url);
    if (this.busy.includes(k)) return false;
    this.busy = [...this.busy, k];
    this.notice = null;
    try {
      const r = await api.webuiOpen(hostId, url, title);
      if (!r.opened) this.notice = { title, hostId, url, text: r.problem ?? "It couldn't open in Nightgrid.", browser: true };
      return r.opened;
    } catch (e) {
      this.notice = { title, hostId, url, text: errorText(e), browser: false };
      return false;
    } finally {
      this.busy = this.busy.filter((x) => x !== k);
    }
  }

  /** Opens the notice's page in the system browser (through a tunnel when it's on a host). */
  async inBrowser(n: WebUiNotice) {
    this.notice = null;
    try {
      if (n.hostId !== null) await tunnels.openThrough(n.hostId, n.url);
      else await openExternal(n.url);
    } catch (e) {
      this.notice = { ...n, text: errorText(e), browser: false };
    }
  }
}

export const webui = new WebUiStore();
