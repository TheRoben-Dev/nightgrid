/** Subnet calculator for the network tools (IPv4). Pure, offline. */

export interface CidrInfo {
  cidr: string;
  network: string;
  broadcast: string;
  mask: string;
  wildcard: string;
  first: string;
  last: string;
  /** Usable host addresses. */
  hosts: number;
  /** Private, CGNAT/tailnet, loopback, link-local or public. */
  scope: string;
}

const toInt = (ip: string): number | null => {
  const p = ip.trim().split(".");
  if (p.length !== 4 || p.some((x) => !/^\d{1,3}$/.test(x) || Number(x) > 255)) return null;
  return p.reduce((n, x) => n * 256 + Number(x), 0);
};
const toIp = (n: number) => [n >>> 24, (n >>> 16) & 255, (n >>> 8) & 255, n & 255].join(".");

function scopeOf(n: number): string {
  const a = n >>> 24;
  const b = (n >>> 16) & 255;
  if (a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168)) return "private";
  if (a === 100 && b >= 64 && b <= 127) return "CGNAT / tailnet";
  if (a === 127) return "loopback";
  if (a === 169 && b === 254) return "link-local";
  if (a >= 224) return "multicast / reserved";
  return "public";
}

/** `192.168.1.77/24`, `10.0.0.0 255.255.0.0` or a bare address (/32). */
export function cidrInfo(input: string): CidrInfo | null {
  const s = input.trim();
  let ip: string;
  let prefix: number;
  const slash = s.split("/");
  const space = s.split(/\s+/);
  if (slash.length === 2) {
    ip = slash[0];
    if (!/^\d{1,2}$/.test(slash[1])) return null;
    prefix = Number(slash[1]);
  } else if (space.length === 2) {
    ip = space[0];
    const m = toInt(space[1]);
    if (m === null) return null;
    const bits = m.toString(2).padStart(32, "0");
    if (!/^1*0*$/.test(bits)) return null;
    prefix = bits.indexOf("0") === -1 ? 32 : bits.indexOf("0");
  } else {
    ip = s;
    prefix = 32;
  }
  const n = toInt(ip);
  if (n === null || prefix < 0 || prefix > 32) return null;
  const mask = prefix === 0 ? 0 : (0xffffffff << (32 - prefix)) >>> 0;
  const net = (n & mask) >>> 0;
  const bc = (net | (~mask >>> 0)) >>> 0;
  const size = 2 ** (32 - prefix);
  const small = prefix >= 31;
  return {
    cidr: `${toIp(net)}/${prefix}`,
    network: toIp(net),
    broadcast: small ? "—" : toIp(bc),
    mask: toIp(mask),
    wildcard: toIp(~mask >>> 0),
    first: toIp(small ? net : net + 1),
    last: toIp(small ? bc : bc - 1),
    hosts: small ? size : size - 2,
    scope: scopeOf(n),
  };
}

/** Whether `ip` is inside `cidr`. */
export function cidrContains(cidr: string, ip: string): boolean {
  const c = cidrInfo(cidr);
  const n = toInt(ip);
  if (!c || n === null) return false;
  const lo = toInt(c.network)!;
  const prefix = Number(c.cidr.split("/")[1]);
  return n - lo >= 0 && n - lo < 2 ** (32 - prefix);
}

/** `AA-BB-CC-DD-EE-FF`, `aabb.ccdd.eeff`, `aabbccddeeff` → `aa:bb:cc:dd:ee:ff`, else null. */
export function normMac(s: string): string | null {
  const hex = s.replace(/[^0-9a-f]/gi, "").toLowerCase();
  if (hex.length !== 12 || /[^0-9a-f:.\-\s]/i.test(s.trim())) return null;
  return hex.match(/../g)!.join(":");
}
