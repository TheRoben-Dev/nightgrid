// Views that load on first use (0.10.48): each is its own chunk, so a view
// that's turned off in Settings → Features (or never opened) is never parsed.
// Keep these references stable: LazyView caches by loader. Store components
// with a pack (ui/tools/storeUi.ts) load through packedView, which uses a
// signed component pack when one is newer than the built-in copy.

import { packedView } from "./componentUi";

export const VIEWS = {
  files: () => import("./views/Files.svelte"),
  docker: () => import("./views/Docker.svelte"),
  system: () => import("./views/System.svelte"),
  services: () => import("./views/Services.svelte"),
  tunnels: () => import("./views/Tunnels.svelte"),
  automation: packedView("automation", () => import("./views/Automation.svelte")),
  fleet: () => import("./views/Fleet.svelte"),
  security: packedView("security", () => import("./views/Security.svelte")),
  desktops: packedView("desktops", () => import("./views/Desktops.svelte")),
  proxmox: packedView("proxmox", () => import("./views/Proxmox.svelte")),
  network: packedView("netmap", () => import("./views/Network.svelte")),
  nettools: () => import("./views/NetTools.svelte"),
  notes: () => import("./views/Notes.svelte"),
  library: () => import("./views/Library.svelte"),
} as const;
