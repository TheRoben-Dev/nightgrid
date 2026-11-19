/** Network tests between hosts (M10x): Fleet → SPEED. */
import { sound } from "./sound.svelte";
import { api, errorText, type NetTest } from "../api";

class NetperfStore {
  tests = $state<NetTest[]>([]);
  loaded = $state(false);
  running = $state<string | null>(null);
  error = $state<string | null>(null);
  /** Finding from the last run, keyed by pair. */
  findings = $state<Record<string, string>>({});

  async load() {
    try {
      this.tests = await api.netTestsList();
      this.loaded = true;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async run(fromHost: number, toHost: number, target: string | null, secs: number) {
    const key = `${fromHost}>${toHost}`;
    if (this.running) return;
    this.running = key;
    this.error = null;
    try {
      const r = await api.netTestRun(fromHost, toHost, target, secs);
      const { [key]: _, ...rest } = this.findings;
      this.findings = r.finding ? { ...rest, [key]: r.finding } : rest;
      await this.load();
      sound.play("taskDone");
    } catch (e) {
      this.error = errorText(e);
      sound.play("taskFail");
    } finally {
      this.running = null;
    }
  }

  async clear(fromHost: number, toHost: number) {
    try {
      await api.netTestsClear(fromHost, toHost);
      await this.load();
    } catch (e) {
      this.error = errorText(e);
    }
  }
}

export const netperf = new NetperfStore();
