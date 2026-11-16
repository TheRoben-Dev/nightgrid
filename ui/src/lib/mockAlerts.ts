/**
 * Browser-only stand-in for alerts: a couple of sample alerts so the panel
 * and status strip can be developed and screenshotted without Tauri.
 */
import type { Alert_Serialize as Alert, Dependency, Node, AlertsSnapshot_Serialize as AlertsSnapshot, TestResult_Serialize as TestResult, WebhookInfo_Serialize as WebhookInfo } from "./bindings";

const now = () => Math.floor(Date.now() / 1000);
let listener: ((s: AlertsSnapshot) => void) | null = null;
let webhook: string | null = null;

const active: Alert[] = [
  { id: 2, key: "disk:1:/", kind: "diskFull", severity: "warn", title: "server01", detail: "/ is 93% full (6.8 GB free)", hostId: 1, since: now() - 1800, resolved: null, causedBy: null },
  { id: 4, key: "host:2", kind: "hostOffline", severity: "critical", title: "server02", detail: "unreachable: connection timed out", hostId: 2, since: now() - 300, resolved: null, causedBy: null },
  { id: 3, key: "docker:2:worker", kind: "containerDown", severity: "critical", title: "worker on server02", detail: "exited with code 137 (killed, maybe out of memory)", hostId: 2, since: now() - 240, resolved: null, causedBy: null },
  { id: 5, key: "svc:3", kind: "serviceDown", severity: "critical", title: "Photos", detail: "connection refused", hostId: null, since: now() - 200, resolved: null, causedBy: null },
];
let deps: Dependency[] = [{ id: 1, child: { kind: "service", id: 3 }, parent: { kind: "host", id: 2 } }];
let nextDep = 2;

/** A rough stand-in for the Rust folding: alerts on an offline host, or on a service that depends on it. */
function fold(list: Alert[]): Alert[] {
  const down = new Map(list.filter((a) => a.kind === "hostOffline" && a.hostId != null).map((a) => [a.hostId as number, a.id]));
  return list.map((a) => {
    if (a.kind === "hostOffline") return a;
    const svc = /^svc:(\d+)/.exec(a.key);
    const dep = svc ? deps.find((d) => d.child.kind === "service" && d.child.id === Number(svc[1]) && d.parent.kind === "host") : null;
    const host = dep ? dep.parent.id : a.hostId;
    return { ...a, causedBy: host != null ? (down.get(host) ?? null) : null };
  });
}
let history: Alert[] = [
  { id: 1, key: "host:2", kind: "hostOffline", severity: "critical", title: "server02", detail: "unreachable: connection refused", hostId: 2, since: now() - 7200, resolved: now() - 6900, causedBy: null },
];

const snapshot = (): AlertsSnapshot => ({ active: fold(active), history: [...history], quiet: false, enabled: true });

export const mockAlerts = {
  async subscribe(on: (s: AlertsSnapshot) => void): Promise<null> {
    listener = on;
    on(snapshot());
    return null;
  },
  async refresh(): Promise<null> {
    listener?.(snapshot());
    return null;
  },
  async clearHistory(): Promise<null> {
    history = [];
    listener?.(snapshot());
    return null;
  },
  async webhookGet(): Promise<WebhookInfo> {
    return info();
  },
  async webhookSet(url: string | null): Promise<WebhookInfo> {
    webhook = url?.trim() || null;
    return info();
  },
  async depsList(): Promise<Dependency[]> {
    return [...deps];
  },
  async depsAdd(child: Node, parent: Node): Promise<Dependency> {
    const same = (a: Node, b: Node) => a.kind === b.kind && a.id === b.id;
    if (same(child, parent)) throw new Error("a host or service can't depend on itself");
    if (deps.some((d) => same(d.child, parent) && same(d.parent, child))) throw new Error("that would make a loop: the other side already depends on this one");
    const hit = deps.find((d) => same(d.child, child) && same(d.parent, parent));
    if (hit) return hit;
    const d = { id: nextDep++, child, parent };
    deps = [...deps, d];
    listener?.(snapshot());
    return d;
  },
  async depsDelete(id: number): Promise<null> {
    deps = deps.filter((d) => d.id !== id);
    listener?.(snapshot());
    return null;
  },
  async test(): Promise<TestResult> {
    return { native: "sent", webhook: webhook ? "delivered (HTTP 200)" : null };
  },
};

function info(): WebhookInfo {
  if (!webhook) return { configured: false, host: null };
  const m = /^(\w+):\/\/(?:[^@/]*@)?([^/?#]+)/.exec(webhook);
  return { configured: true, host: m ? `${m[1]}://${m[2]}` : null };
}
