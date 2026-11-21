/**
 * Browser-only stand-in for transmissions (D-202): nothing by default, so
 * previews and tests aren't covered by a dialog. Open the preview with
 * `?txdemo` (or `?txdemo=critical`) to see an INCOMING TRANSMISSION.
 */
import type { Broadcast, BroadcastStatus } from "./api";

const base = { link: null, os: [], minApp: null, maxApp: null, features: [], starts: null, expires: null };

const DEMO: Broadcast[] = [
  {
    ...base,
    id: "2026-11-21-tailscale-reauth",
    level: "warn",
    title: "Built-in Tailscale: sign in again before Monday",
    body:
      "Tailscale is retiring the login method Nightgrid's built-in client used before **1.22**. Until you sign in again, hosts reached only over the tailnet will show as offline.\n\n" +
      "- Open **Settings → Connections** and press **Sign out**\n- Press **Sign in** and approve Nightgrid in the browser\n- Check that your tailnet hosts come back in [Fleet](view:fleet)\n\n" +
      "Nothing else changes: your hosts, keys and settings stay as they are.",
    link: { label: "Open Connections", to: "view:settings/connections" },
  },
  {
    ...base,
    id: "2026-11-20-docker-29",
    level: "info",
    title: "Docker 29 hosts: update the compose plugin",
    body: "Stacks on hosts that upgraded to Docker 29 with an old compose plugin show **no stacks**. Update `docker-compose-plugin` on the host and press **Refresh**.",
    link: { label: "Read the guide", to: "kb:howto/update-docker" },
  },
];

function demo(): Broadcast[] {
  if (typeof location === "undefined") return [];
  const q = new URLSearchParams(location.search);
  if (!q.has("txdemo")) return [];
  return q.get("txdemo") === "critical" ? [{ ...DEMO[0], level: "critical" }, DEMO[1]] : DEMO;
}

export const mockBroadcast = {
  status(): BroadcastStatus {
    const notices = demo();
    return { notices, version: notices.length ? 1795219200 : 0, checkedAt: null, error: null, notConfigured: !notices.length };
  },
};
