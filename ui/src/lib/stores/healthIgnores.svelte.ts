/**
 * Ignored healthchecks (D-201): containers that are "running but unhealthy"
 * by design. The backend marks them on every container list
 * (`ContainerView.healthIgnored`) and stops alerting on them; this store
 * keeps the list for the toggle and for places that only have an inspect
 * result (alert triage).
 */
import { api, errorText, type ContainerDetail, type ContainerView, type HealthIgnore } from "../api";
import { docker } from "./docker.svelte";

const LABEL_PROJECT = "com.docker.compose.project";
const LABEL_SERVICE = "com.docker.compose.service";

/** The health Nightgrid judges a container by: null when ignored. */
export const effectiveHealth = (c: Pick<ContainerView, "health" | "healthIgnored">): string | null => (c.healthIgnored ? null : c.health);

/** Docker's health, with "check ignored" when the user ignores it (for DRIFT's context). */
export const healthText = (c: Pick<ContainerView, "health" | "healthIgnored">): string | null =>
  c.healthIgnored ? `${c.health ?? "health"}, check ignored` : c.health;

/** Whether an ignore covers a container (by compose service, else by name). */
export function ignoreMatches(i: HealthIgnore, hostId: number, c: { name: string; project: string | null; service: string | null }): boolean {
  if (i.hostId !== hostId) return false;
  return i.project ? c.project === i.project && c.service === i.service : !!i.name && i.name === c.name.replace(/^\//, "");
}

class HealthIgnoreStore {
  list = $state<HealthIgnore[]>([]);
  loaded = $state(false);
  busy = $state(false);

  async load() {
    try {
      this.list = await api.dockerHealthIgnores();
      this.loaded = true;
    } catch {
      /* best effort */
    }
  }

  /** Whether an inspected container's healthcheck is ignored. */
  ignoredDetail(hostId: number, d: ContainerDetail): boolean {
    const label = (k: string) => d.labels.find((l) => l.key === k)?.value ?? null;
    return this.list.some((i) => ignoreMatches(i, hostId, { name: d.name, project: label(LABEL_PROJECT), service: label(LABEL_SERVICE) }));
  }

  /** Turns ignoring on or off for one container, then refreshes the Docker list. */
  async set(hostId: number, c: ContainerView, ignored: boolean) {
    this.busy = true;
    const who = c.service && c.project ? c.service : c.name;
    try {
      await api.dockerSetHealthIgnored({ hostId, project: c.project, service: c.service, name: c.name }, ignored);
      await this.load();
      docker.notice = {
        text: ignored ? `Health check ignored for ${who}: unhealthy no longer alerts (crashes still do)` : `Watching ${who}'s health check again`,
        tone: "ok",
      };
      if (docker.hostId === hostId) await docker.refresh(true);
    } catch (e) {
      docker.notice = { text: errorText(e), tone: "err" };
    } finally {
      this.busy = false;
    }
  }
}

export const healthIgnores = new HealthIgnoreStore();
