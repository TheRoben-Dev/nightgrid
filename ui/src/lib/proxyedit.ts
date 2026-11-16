/** Reverse proxy route editor helpers (M10v). */
import type { CaddyEdit } from "./api";

/** A short description of a Caddyfile change, for the confirmation and the log. */
export function editSummary(e: CaddyEdit): string {
  switch (e.op) {
    case "addSite":
      return `add site ${e.domains.join(", ")} → ${e.upstream}`;
    case "setUpstream":
      return `send ${e.site} to ${e.upstream}`;
    case "removeSite":
      return `remove site ${e.site}`;
    case "replace":
      return "edit the Caddyfile";
  }
}

/** "a.example.com, b.example.com" or "a b" → list. */
export function splitDomains(s: string): string[] {
  return s
    .split(/[\s,]+/)
    .map((d) => d.trim())
    .filter(Boolean);
}

export const targetKey = (kind: "caddy" | "traefik", name: string) => `${kind}:${name}`;
