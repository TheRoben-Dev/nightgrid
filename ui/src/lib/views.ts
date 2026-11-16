// Views that load on first use (0.10.48): each is its own chunk, so a view
// that's turned off in Settings → Features (or never opened) is never parsed.
// Keep these references stable: LazyView caches by loader.

export const VIEWS = {
  files: () => import("./views/Files.svelte"),
  docker: () => import("./views/Docker.svelte"),
  system: () => import("./views/System.svelte"),
  services: () => import("./views/Services.svelte"),
  tunnels: () => import("./views/Tunnels.svelte"),
  automation: () => import("./views/Automation.svelte"),
  fleet: () => import("./views/Fleet.svelte"),
  desktops: () => import("./views/Desktops.svelte"),
  proxmox: () => import("./views/Proxmox.svelte"),
  network: () => import("./views/Network.svelte"),
  notes: () => import("./views/Notes.svelte"),
} as const;
