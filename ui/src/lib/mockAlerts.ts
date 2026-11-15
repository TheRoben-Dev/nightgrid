/**
 * Browser-only stand-in for alerts: a couple of sample alerts so the panel
 * and status strip can be developed and screenshotted without Tauri.
 */
import type { Alert_Serialize as Alert, AlertsSnapshot_Serialize as AlertsSnapshot, TestResult_Serialize as TestResult, WebhookInfo_Serialize as WebhookInfo } from "./bindings";

const now = () => Math.floor(Date.now() / 1000);
let listener: ((s: AlertsSnapshot) => void) | null = null;
let webhook: string | null = null;

const active: Alert[] = [
  { id: 2, key: "disk:1:/", kind: "diskFull", severity: "warn", title: "server01", detail: "/ is 93% full (6.8 GB free)", hostId: 1, since: now() - 1800, resolved: null },
  { id: 3, key: "docker:2:worker", kind: "containerDown", severity: "critical", title: "worker on server02", detail: "exited with code 137 (killed, maybe out of memory)", hostId: 2, since: now() - 240, resolved: null },
];
let history: Alert[] = [
  { id: 1, key: "host:2", kind: "hostOffline", severity: "critical", title: "server02", detail: "unreachable: connection refused", hostId: 2, since: now() - 7200, resolved: now() - 6900 },
];

const snapshot = (): AlertsSnapshot => ({ active: [...active], history: [...history], quiet: false, enabled: true });

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
  async test(): Promise<TestResult> {
    return { native: "sent", webhook: webhook ? "delivered (HTTP 200)" : null };
  },
};

function info(): WebhookInfo {
  if (!webhook) return { configured: false, host: null };
  const m = /^(\w+):\/\/(?:[^@/]*@)?([^/?#]+)/.exec(webhook);
  return { configured: true, host: m ? `${m[1]}://${m[2]}` : null };
}
