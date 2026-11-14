import { SECTIONS } from "../sections";
import { settings } from "../stores/settings.svelte";
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

  return [...nav, ...view, ...themes];
}
