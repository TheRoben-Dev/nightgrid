/**
 * Whether an address is on a tailnet: 100.64.0.0/10, Tailscale's IPv6
 * range fd7a:115c:a1e0::/48, or a MagicDNS name (`*.ts.net`). Used for hints
 * only; routing is decided by the backend.
 */
export function looksTailnet(addr: string | null | undefined): boolean {
  const a = (addr ?? "").trim().toLowerCase().replace(/^\[|\]$/g, "").replace(/\.$/, "");
  if (!a) return false;
  if (a.endsWith(".ts.net")) return true;
  if (a.startsWith("fd7a:115c:a1e0:")) return true;
  const m = /^(\d{1,3})\.(\d{1,3})\.\d{1,3}\.\d{1,3}$/.exec(a);
  return !!m && Number(m[1]) === 100 && Number(m[2]) >= 64 && Number(m[2]) <= 127;
}
