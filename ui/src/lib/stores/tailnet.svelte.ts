/**
 * Built-in Tailscale client (M10j): its status (pushed by the backend on
 * `tailnet://status`) and the actions behind Settings → Tailnet.
 */
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@tauri-apps/api/core";
import { api, errorText, type TailnetStatus } from "../api";
import { openExternal } from "../clipboard";
import { mockTailnet } from "../mockTailnet";

class TailnetStore {
  status = $state<TailnetStatus | null>(null);
  busy = $state(false);
  error = $state<string | null>(null);
  /** Open the login page as soon as its link arrives (after the user clicked Log in). */
  private openNextLogin = false;
  private started = false;

  readonly on = $derived(!!this.status && !["off", "unavailable", "failed", "stopped"].includes(this.status.state));
  readonly running = $derived(this.status?.state === "running");

  start() {
    if (this.started) return;
    this.started = true;
    const take = (s: TailnetStatus) => {
      this.status = s;
      if (this.openNextLogin && s.loginUrl) {
        this.openNextLogin = false;
        void openExternal(s.loginUrl);
      }
    };
    if (isTauri()) void listen<TailnetStatus>("tailnet://status", (e) => take(e.payload));
    else mockTailnet.subscribe(take);
    void api.tailnetStatus().then(take, (e) => (this.error = errorText(e)));
  }

  private async run<T>(f: () => Promise<T>): Promise<T | undefined> {
    this.busy = true;
    this.error = null;
    try {
      return await f();
    } catch (e) {
      this.error = errorText(e);
      return undefined;
    } finally {
      this.busy = false;
    }
  }

  async setEnabled(on: boolean) {
    const s = await this.run(() => api.tailnetSetEnabled(on));
    if (s) this.status = s;
  }

  async configure(hostname: string, controlUrl: string, acceptRoutes: boolean) {
    const s = await this.run(() => api.tailnetConfigure(hostname, controlUrl, acceptRoutes));
    if (s) this.status = s;
    return !!s;
  }

  async login() {
    if (this.status?.loginUrl) return openExternal(this.status.loginUrl);
    this.openNextLogin = true;
    await this.run(() => api.tailnetLogin());
  }

  async logout() {
    await this.run(() => api.tailnetLogout());
  }
}

export const tailnet = new TailnetStore();
