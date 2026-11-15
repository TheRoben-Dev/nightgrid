/** Helpers for the phone status page settings (M10h). */

export const BIND_LABEL: Record<string, string> = {
  lan: "This machine's LAN address",
  tailscale: "Tailscale only (100.x)",
  all: "Every interface",
};

/** Hides the token in a status page link (rule 6: masked until revealed). */
export function maskUrl(u: string): string {
  return u.replace(/\/s\/([0-9a-f]+)/i, (_m, t: string) => `/s/${t.slice(0, 4)}${"•".repeat(12)}`);
}
