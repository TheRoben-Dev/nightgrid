/** Cron and timer editor (M10v): System → SCHEDULES on the current host. */
import { api, errorText, type CronEdit, type Crontab, type InstalledJob, type TimerJob } from "../api";
import { system } from "./system.svelte";

class JobsStore {
  /** The crontab open in the editor. */
  tab = $state<Crontab | null>(null);
  tabUser = $state<string | null>(null);
  tabBusy = $state(false);
  tabError = $state<string | null>(null);
  timers = $state<InstalledJob[]>([]);
  timerError = $state<string | null>(null);

  private host(): number | null {
    return system.hostId;
  }

  private async sudoIf<T>(needs: boolean, reason: string, f: (sudo: Parameters<typeof api.cronRead>[2]) => Promise<T>): Promise<T> {
    return needs ? system.withSudo(reason, f) : f(null);
  }

  /** `user` null: the login user's crontab. */
  async openTab(user: string | null) {
    const id = this.host();
    if (id === null) return;
    this.tabUser = user;
    this.tab = null;
    this.tabError = null;
    this.tabBusy = true;
    try {
      this.tab = await this.sudoIf(user !== null, `read ${user}'s crontab`, (sudo) => api.cronRead(id, user, sudo));
    } catch (e) {
      this.tabError = errorText(e);
    }
    this.tabBusy = false;
  }

  /** Applies one change; true when it worked. */
  async edit(edit: CronEdit): Promise<boolean> {
    const id = this.host();
    const t = this.tab;
    if (id === null || !t) return false;
    this.tabBusy = true;
    this.tabError = null;
    let ok = false;
    try {
      this.tab = await this.sudoIf(t.user !== null, `change ${t.user}'s crontab`, (sudo) => api.cronApply(id, t.user, t.hash, edit, sudo));
      ok = true;
      void system.loadSchedules();
    } catch (e) {
      this.tabError = errorText(e);
    }
    this.tabBusy = false;
    return ok;
  }

  async loadTimers() {
    const id = this.host();
    if (id === null) return;
    try {
      this.timers = await api.timerJobs(id);
      this.timerError = null;
    } catch (e) {
      this.timerError = errorText(e);
    }
  }

  /** Saves a timer; returns an error text or null. */
  async saveTimer(job: TimerJob, replaces: string | null): Promise<string | null> {
    const id = this.host();
    if (id === null) return "no host";
    try {
      await system.withSudo(`install the timer ${job.name}`, (sudo) => api.timerJobSave(id, job, replaces, sudo));
      await Promise.all([this.loadTimers(), system.loadSchedules()]);
      return null;
    } catch (e) {
      return errorText(e);
    }
  }

  async removeTimer(unit: string): Promise<string | null> {
    const id = this.host();
    if (id === null) return "no host";
    try {
      await system.withSudo(`remove the timer ${unit}`, (sudo) => api.timerJobRemove(id, unit, sudo));
      await Promise.all([this.loadTimers(), system.loadSchedules()]);
      return null;
    } catch (e) {
      return errorText(e);
    }
  }
}

export const jobs = new JobsStore();
