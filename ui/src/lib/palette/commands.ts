import { openExternal } from "../clipboard";
import { SECTIONS } from "../sections";
import { automation } from "../stores/automation.svelte";
import { docker } from "../stores/docker.svelte";
import { files } from "../stores/files.svelte";
import { hosts } from "../stores/hosts.svelte";
import { layout } from "../stores/layout.svelte";
import { metrics } from "../stores/metrics.svelte";
import { services } from "../stores/services.svelte";
import { isActive, tunnels } from "../stores/tunnels.svelte";
import { system, type SystemTab } from "../stores/system.svelte";
import { settings } from "../stores/settings.svelte";
import { terms } from "../stores/terms.svelte";
import { ui } from "../stores/ui.svelte";
import { helper } from "../stores/helper.svelte";
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
      id: "helper.toggle",
      title: helper.open ? "Close DRIFT helper" : "Ask DRIFT (helper)",
      group: "VIEW",
      keywords: ["help", "assistant", "ai", "llm", "explain", "question", "chat"],
      action: "helper.toggle",
      run: () => helper.toggle(),
    },
    {
      id: "layout.reset",
      title: "Reset layout (panel sizes)",
      group: "VIEW",
      keywords: ["resize", "sidebar width", "panes", "borders", "splitter"],
      run: () => layout.reset(),
    },
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

  const monitorHosts: PaletteCommand[] = hosts.hosts.map((h) => ({
    id: `monitor.${h.id}`,
    title: `Monitor: ${h.name}`,
    group: "MONITOR",
    keywords: ["metrics", "cpu", "memory", "disk", "temperature", "history", h.hostname],
    run: () => {
      metrics.detailId = h.id;
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

  const serviceCmds: PaletteCommand[] = [
    {
      id: "services.add",
      title: "Add service",
      group: "SERVICES",
      keywords: ["web app", "url", "health check", "uptime"],
      run: () => {
        ui.go("services");
        services.editing = 0;
      },
    },
    {
      id: "services.import",
      title: "Import Homepage services.yaml",
      group: "SERVICES",
      keywords: ["gethomepage", "dashboard", "yaml", "sync"],
      run: () => {
        ui.go("services");
        services.importOpen = true;
      },
    },
    ...services.services.map((s) => ({
      id: `services.open.${s.id}`,
      title: `Open ${s.name}`,
      group: "SERVICES",
      keywords: [s.url, s.group],
      run: () => void openExternal(s.url),
    })),
  ];

  const tunnelCmds: PaletteCommand[] = [
    ...(hosts.hosts.length
      ? [
          {
            id: "tunnels.add",
            title: "Add tunnel (port forward)",
            group: "TUNNELS",
            keywords: ["ssh -L", "-R", "-D", "socks", "forward", "port"],
            run: () => {
              ui.go("tunnels");
              tunnels.openNew(hosts.hosts[0].id);
            },
          },
        ]
      : []),
    ...tunnels.tunnels.map((t) => {
      const on = isActive(tunnels.stateOf(t.id));
      return {
        id: `tunnels.toggle.${t.id}`,
        title: `${on ? "Stop" : "Start"} tunnel ${t.name || t.spec}`,
        group: "TUNNELS",
        keywords: [t.spec, hosts.byId(t.hostId)?.name ?? ""],
        run: () => void tunnels.toggle(t.id),
      };
    }),
  ];

  const systemTabs: [SystemTab, string, string[]][] = [
    ["units", "systemd units", ["services", "systemctl", "restart"]],
    ["journal", "journal (logs)", ["journalctl", "logs", "syslog"]],
    ["updates", "package updates", ["apt", "dnf", "pacman", "upgrade", "reboot"]],
    ["disks", "disk health (SMART)", ["smartctl", "smart", "drive"]],
  ];
  const systemCmds: PaletteCommand[] = hosts.hosts.flatMap((h) =>
    systemTabs.map(([tab, label, keywords]) => ({
      id: `system.${tab}.${h.id}`,
      title: `${h.name}: ${label}`,
      group: "SYSTEM",
      keywords: [...keywords, h.hostname],
      run: () => {
        ui.go("system");
        system.select(h.id);
        void system.openTab(tab);
      },
    })),
  );

  // Snippets: offered for the host of the terminal in front, if any.
  const session = terms.currentSession;
  const termHostId = session?.origin.kind === "ssh" ? session.origin.hostId : undefined;
  const termHost = termHostId !== undefined ? hosts.byId(termHostId) : undefined;
  const inTerminal = ui.section === "terminal" && session?.state === "connected";
  const snippetCmds: PaletteCommand[] = automation.snippetsFor(termHost).flatMap((s) => {
    const out: PaletteCommand[] = [
      {
        id: `snippet.${s.id}`,
        title: `Snippet: ${s.name}${termHost ? ` on ${termHost.name}` : ""}`,
        group: "SNIPPETS",
        keywords: ["run", "automation", s.command, s.description],
        run: () => automation.openSnippet(s.id, termHostId),
      },
    ];
    // Plain snippets can go straight to the prompt (Enter still runs them).
    if (inTerminal && session && !s.sudo && !s.command.includes("{{")) {
      out.push({
        id: `snippet.type.${s.id}`,
        title: `Type into terminal: ${s.name}`,
        group: "SNIPPETS",
        keywords: ["paste", "insert", s.command],
        run: () => session.paste(s.command),
      });
    }
    return out;
  });
  const automationCmds: PaletteCommand[] = [
    {
      id: "automation.command",
      title: "Run a command on several hosts",
      group: "AUTOMATION",
      keywords: ["multi-host", "parallel", "broadcast", "ssh", "exec"],
      run: () => {
        automation.pick({ kind: "command" });
        if (termHostId !== undefined) automation.setTargets([termHostId]);
        ui.go("automation");
      },
    },
    { id: "automation.newSnippet", title: "New snippet", group: "AUTOMATION", keywords: ["save command"], run: () => (automation.editingSnippet = 0) },
    { id: "automation.newPlaybook", title: "New playbook", group: "AUTOMATION", keywords: ["steps", "deploy"], run: () => (automation.editingPlaybook = 0) },
    ...automation.playbooks.map((p) => ({
      id: `playbook.${p.id}`,
      title: `Playbook: ${p.name}`,
      group: "AUTOMATION",
      keywords: ["run", "deploy", p.description],
      run: () => {
        automation.pick({ kind: "playbook", id: p.id });
        ui.go("automation");
      },
    })),
  ];

  return [...snippetCmds, ...automationCmds, ...connect, ...browse, ...dockerHosts, ...tunnelCmds, ...systemCmds,
    ...stackHosts, ...monitorHosts, ...containers, ...hostCmds, ...serviceCmds, ...nav, ...view, ...themes];
}
