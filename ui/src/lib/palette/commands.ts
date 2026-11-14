import { SECTIONS } from "../sections";
import { docker } from "../stores/docker.svelte";
import { files } from "../stores/files.svelte";
import { hosts } from "../stores/hosts.svelte";
import { settings } from "../stores/settings.svelte";
import { terms } from "../stores/terms.svelte";
import { ui } from "../stores/ui.svelte";
import { THEME_IDS, THEMES } from "../theme/themes";
import type { PaletteCommand } from "./types";

/**
 * The palette's command list. Later milestones add hosts, snippets,
 * containers and tunnels here.
 */
export function buildCommands(): PaletteCommand[] {
  const nav: PaletteCommand[] = SECTIONS.map((s) => ({
    id: `nav.${s.id}`,
    title: `Go to ${s.label}`,
    group: "NAVIGATE",
    keywords: [s.code],
    action: s.action,
    run: () => ui.go(s.id),
  }));

  const themes: PaletteCommand[] = THEME_IDS.map((id) => ({
    id: `theme.${id}`,
    title: `Theme: ${THEMES[id].label}`,
    group: "THEME",
    keywords: ["color", "colour", "appearance"],
    run: () => settings.update({ theme: id }),
  }));

  const view: PaletteCommand[] = [
    {
      id: "effects.toggle",
      title: settings.value.reduceEffects ? "Enable effects (glow, scanlines, glitch)" : "Reduce effects (glow, scanlines, glitch)",
      group: "VIEW",
      keywords: ["motion", "animation", "accessibility"],
      action: "effects.toggle",
      run: () => settings.update({ reduceEffects: !settings.value.reduceEffects }),
    },
    {
      id: "scanlines.toggle",
      title: settings.value.scanlines ? "Hide scanline overlay" : "Show scanline overlay",
      group: "VIEW",
      keywords: ["crt"],
      run: () => settings.update({ scanlines: !settings.value.scanlines }),
    },
    {
      id: "rail.toggle",
      title: settings.value.railCollapsed ? "Expand sidebar" : "Collapse sidebar",
      group: "VIEW",
      keywords: ["rail", "sidebar"],
      action: "rail.toggle",
      run: () => settings.update({ railCollapsed: !settings.value.railCollapsed }),
    },
  ];

  const connect: PaletteCommand[] = hosts.hosts.map((h) => ({
    id: `ssh.${h.id}`,
    title: `SSH: ${h.name}`,
    group: "CONNECT",
    keywords: [h.hostname, h.username ?? "", ...h.tags],
    run: () => terms.openHost(h.id),
  }));

  const browse: PaletteCommand[] = hosts.hosts.map((h) => ({
    id: `sftp.${h.id}`,
    title: `Files: ${h.name}`,
    group: "FILES",
    keywords: ["sftp", "browse", h.hostname],
    run: () => {
      ui.go("files");
      void files.browseHost(h.id);
    },
  }));

  const dockerHosts: PaletteCommand[] = hosts.hosts.map((h) => ({
    id: `docker.${h.id}`,
    title: `Docker: ${h.name}`,
    group: "DOCKER",
    keywords: ["containers", "compose", h.hostname],
    run: () => {
      docker.select(h.id);
      docker.tab = "containers";
      ui.go("docker");
    },
  }));

  const stackHosts: PaletteCommand[] = hosts.hosts.map((h) => ({
    id: `stacks.${h.id}`,
    title: `Stacks: ${h.name}`,
    group: "DOCKER",
    keywords: ["compose", "docker compose", "up", "pull", "update", h.hostname],
    run: () => {
      docker.select(h.id);
      docker.tab = "stacks";
      ui.go("docker");
    },
  }));

  // Containers already loaded for the current Docker host.
  const containers: PaletteCommand[] = docker.containers.flatMap((c) => [
    {
      id: `docker.logs.${c.id}`,
      title: `Logs: ${c.name}`,
      group: "DOCKER",
      keywords: [c.image, c.project ?? "", "container"],
      run: () => {
        docker.detailId = c.id;
        ui.go("docker");
      },
    },
    ...(c.state === "running" && docker.hostId !== null
      ? [
          {
            id: `docker.shell.${c.id}`,
            title: `Shell: ${c.name}`,
            group: "DOCKER",
            keywords: [c.image, "exec", "container"],
            run: () => terms.openContainer(docker.hostId!, { id: c.id, name: c.name }),
          },
        ]
      : []),
  ]);

  const hostCmds: PaletteCommand[] = [
    {
      id: "term.newLocal",
      title: "New local terminal",
      group: "TERMINAL",
      keywords: ["shell", "pty"],
      action: "term.newLocal",
      run: () => terms.openLocal(),
    },
    { id: "host.add", title: "Add host", group: "HOSTS", keywords: ["new", "node"], run: () => void (hosts.editing = 0) },
    {
      id: "host.import",
      title: "Import hosts from ~/.ssh/config",
      group: "HOSTS",
      keywords: ["ssh config", "sync"],
      run: () => void (hosts.importOpen = true),
    },
    ...hosts.hosts.map((h) => ({
      id: `host.edit.${h.id}`,
      title: `Edit host: ${h.name}`,
      group: "HOSTS",
      keywords: [h.hostname],
      run: () => void (hosts.editing = h.id),
    })),
  ];

  return [...connect, ...browse, ...dockerHosts,
    ...stackHosts, ...containers, ...hostCmds, ...nav, ...view, ...themes];
}
