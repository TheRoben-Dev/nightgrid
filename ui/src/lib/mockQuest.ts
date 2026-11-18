/** Browser-only stand-in for quest checks (M12e): a believable half-done host. */
import type { QuestResult } from "./bindings";

const PASS = new Set(["ssh-key", "docker", "docker-group", "time-sync", "swap", "container:caddy", "port:443", "port:53", "no-reboot-pending", "tailscale"]);
const UNKNOWN = new Set(["ssh-no-root-password"]);

export const mockQuest = {
  async check(checks: string[]): Promise<QuestResult[]> {
    await new Promise((r) => setTimeout(r, 400));
    return [...new Set(checks)].map((id) =>
      PASS.has(id)
        ? { id, state: "pass", detail: id.startsWith("container:") ? `${id.slice(10)} running` : "ok" }
        : UNKNOWN.has(id)
          ? { id, state: "unknown", detail: "/etc/ssh/sshd_config.d/50-cloud-init.conf needs root to read" }
          : { id, state: "fail", detail: id === "ssh-no-password" ? "passwordauthentication yes (default)" : "not set up" },
    );
  },
};
