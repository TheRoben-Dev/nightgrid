/**
 * Login notices from servers while a host connects (0.10.51): Tailscale SSH
 * in "check" mode sends an approval link and holds the login until it's
 * approved in a browser. Polled while the window is visible; the backend
 * drops a notice as soon as the host is connected (or gave up).
 */
import { api, type HostAuthNotice } from "../api";

class AuthNoticesStore {
  list = $state<HostAuthNotice[]>([]);
  /** Hidden by the user: `hostId|url`. */
  dismissed = $state<string[]>([]);
  private timer: ReturnType<typeof setInterval> | null = null;

  key = (n: HostAuthNotice) => `${n.hostId}|${n.notice.url ?? n.notice.text}`;

  /** Notices to show, newest host first. */
  get shown(): HostAuthNotice[] {
    return this.list.filter((n) => !this.dismissed.includes(this.key(n)));
  }

  forHost(id: number): HostAuthNotice | undefined {
    return this.list.find((n) => n.hostId === id);
  }

  dismiss(n: HostAuthNotice) {
    this.dismissed = [...this.dismissed, this.key(n)];
  }

  async poll() {
    if (typeof document !== "undefined" && document.hidden) return;
    try {
      const next = await api.sshAuthNotices();
      // Keep dismissals only for notices that are still there.
      const keys = next.map((n) => this.key(n));
      if (this.dismissed.some((d) => !keys.includes(d))) this.dismissed = this.dismissed.filter((d) => keys.includes(d));
      if (next.length || this.list.length) this.list = next;
    } catch {
      /* polling: try again next time */
    }
  }

  start(everyMs = 1500) {
    if (this.timer) return;
    void this.poll();
    this.timer = setInterval(() => void this.poll(), everyMs);
  }

  stop() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }
}

export const authNotices = new AuthNoticesStore();
