/**
 * Titles of every library article (M11), for the command palette and the
 * "?" help links. Small on purpose: the article text lives in library.ts,
 * which loads only with the Library view. A test keeps this list in step
 * with the articles.
 */
import type { SectionId } from "../sections";

export type Shelf = "nightgrid" | "basics" | "guides";

export const SHELVES: { id: Shelf; label: string; what: string }[] = [
  { id: "nightgrid", label: "USING NIGHTGRID", what: "one article per feature" },
  { id: "basics", label: "HOMELAB BASICS", what: "the background the features assume" },
  { id: "guides", label: "GUIDES", what: "whole setups, step by step, with your progress kept" },
];

export const CATALOG: { id: string; shelf: Shelf; title: string }[] = [
  { id: "nightgrid/getting-around", shelf: "nightgrid", title: "Getting around" },
  { id: "nightgrid/hosts", shelf: "nightgrid", title: "Hosts" },
  { id: "nightgrid/terminal", shelf: "nightgrid", title: "Terminal" },
  { id: "nightgrid/files", shelf: "nightgrid", title: "Files" },
  { id: "nightgrid/docker", shelf: "nightgrid", title: "Docker" },
  { id: "nightgrid/proxmox", shelf: "nightgrid", title: "Proxmox" },
  { id: "nightgrid/overview-and-monitoring", shelf: "nightgrid", title: "Overview and monitoring" },
  { id: "nightgrid/services-and-integrations", shelf: "nightgrid", title: "Services and integrations" },
  { id: "nightgrid/tunnels", shelf: "nightgrid", title: "Tunnels" },
  { id: "nightgrid/system", shelf: "nightgrid", title: "System" },
  { id: "nightgrid/fleet", shelf: "nightgrid", title: "Fleet" },
  { id: "nightgrid/desktops", shelf: "nightgrid", title: "Desktops" },
  { id: "nightgrid/automation", shelf: "nightgrid", title: "Automation" },
  { id: "nightgrid/knowledge-notes-and-runbooks", shelf: "nightgrid", title: "Knowledge (notes and runbooks)" },
  { id: "nightgrid/network-map", shelf: "nightgrid", title: "Network map" },
  { id: "nightgrid/drift-helper", shelf: "nightgrid", title: "DRIFT helper" },
  { id: "nightgrid/power", shelf: "nightgrid", title: "Power" },
  { id: "nightgrid/built-in-tailscale", shelf: "nightgrid", title: "Built-in Tailscale" },
  { id: "nightgrid/ssh-keys", shelf: "nightgrid", title: "SSH keys" },
  { id: "nightgrid/alerts-and-the-tray", shelf: "nightgrid", title: "Alerts and the tray" },
  { id: "nightgrid/backups-and-moving-between-machines", shelf: "nightgrid", title: "Backups and moving between machines" },
  { id: "nightgrid/phone-status-page-and-snapshots", shelf: "nightgrid", title: "Phone status page and snapshots" },
  { id: "nightgrid/handhelds-rog-ally-steam-deck-and-controllers", shelf: "nightgrid", title: "Handhelds (ROG Ally, Steam Deck) and controllers" },
  { id: "nightgrid/which-host-a-view-opens-on", shelf: "nightgrid", title: "Which host a view opens on" },
  { id: "nightgrid/features-simple-or-everything", shelf: "nightgrid", title: "Features: simple or everything" },
  { id: "nightgrid/look-and-feel", shelf: "nightgrid", title: "Look and feel" },
  { id: "nightgrid/updates", shelf: "nightgrid", title: "Updates" },
  { id: "nightgrid/privacy", shelf: "nightgrid", title: "Privacy" },
  { id: "nightgrid/troubleshooting", shelf: "nightgrid", title: "Troubleshooting" },
  { id: "basics/backups", shelf: "basics", title: "Backups" },
  { id: "basics/docker-compose", shelf: "basics", title: "Docker and compose basics" },
  { id: "basics/docker-group", shelf: "basics", title: "The docker group" },
  { id: "basics/reading-logs", shelf: "basics", title: "Reading logs" },
  { id: "basics/reverse-proxies", shelf: "basics", title: "Reverse proxies" },
  { id: "basics/ssh-keys", shelf: "basics", title: "SSH keys and agents" },
  { id: "basics/sshd-hardening", shelf: "basics", title: "Hardening sshd" },
  { id: "basics/tailscale-wireguard", shelf: "basics", title: "Tailscale and WireGuard" },
  { id: "guides/dev-git-deploy-lab", shelf: "guides", title: "Dev and git deploy lab" },
  { id: "guides/family-cloud-photos", shelf: "guides", title: "Family cloud and photos" },
  { id: "guides/game-server-host", shelf: "guides", title: "Game server host" },
  { id: "guides/local-ai-gpu-server", shelf: "guides", title: "Local AI GPU server" },
  { id: "guides/proxmox-media-server", shelf: "guides", title: "Proxmox media server, run from Nightgrid" },
  { id: "guides/remote-family-it", shelf: "guides", title: "Remote family IT" },
  { id: "guides/repair-bench-server", shelf: "guides", title: "Repair bench server" },
  { id: "guides/smart-home-hub", shelf: "guides", title: "Smart home hub" },
];

/** The article behind each view's "?" link. */
export const VIEW_HELP: Record<SectionId, string> = {
  overview: "nightgrid/overview-and-monitoring",
  terminal: "nightgrid/terminal",
  files: "nightgrid/files",
  docker: "nightgrid/docker",
  services: "nightgrid/services-and-integrations",
  tunnels: "nightgrid/tunnels",
  system: "nightgrid/system",
  fleet: "nightgrid/fleet",
  desktops: "nightgrid/desktops",
  proxmox: "nightgrid/proxmox",
  network: "nightgrid/network-map",
  nettools: "nightgrid/network-map",
  automation: "nightgrid/automation",
  notes: "nightgrid/knowledge-notes-and-runbooks",
  library: "nightgrid/knowledge-notes-and-runbooks",
  settings: "nightgrid/features-simple-or-everything",
};
