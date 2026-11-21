/**
 * Settings tabs (0.10.54): which tab holds which panel, so DRIFT's spotlights
 * and tours open the right one. Every settings guide target must map here
 * (a test checks it).
 */
export type SettingsTab = "general" | "appearance" | "sound" | "terminal" | "monitoring" | "alerts" | "connections" | "drift" | "sync" | "data" | "features";

export const SETTINGS_TABS: { id: SettingsTab; label: string; what: string }[] = [
  { id: "general", label: "GENERAL", what: "tray and window, app updates, diagnostics" },
  { id: "appearance", label: "APPEARANCE", what: "theme, custom colours, scale and effects" },
  { id: "sound", label: "SOUND", what: "UI sounds: volume, which sounds play, quiet hours, test buttons" },
  { id: "terminal", label: "TERMINAL", what: "terminal font, scrollback, copy and paste, keyboard shortcuts" },
  { id: "monitoring", label: "MONITORING", what: "metrics polling and history, service checks" },
  { id: "alerts", label: "ALERTS", what: "alert rules, log alert rules and mutes, dependencies, notifications" },
  { id: "connections", label: "CONNECTIONS", what: "built-in Tailscale and SSH keys" },
  { id: "drift", label: "DRIFT", what: "the helper, its model and the tours" },
  { id: "sync", label: "SYNC", what: "pair your devices and keep their profiles in step: push, pull or automatic two-way sync" },
  { id: "data", label: "DATA", what: "profile export and import, phone status page" },
  { id: "features", label: "FEATURES", what: "turn views and checks on or off" },
];

/** Guide target id → the tab it's on. */
const BY_TARGET: Record<string, SettingsTab> = {
  "settings-theme": "appearance",
  "settings-display": "appearance",
  "settings-sound": "sound",
  "settings-sound-list": "sound",
  "settings-terminal": "terminal",
  "settings-keybindings": "terminal",
  "settings-monitoring": "monitoring",
  "settings-services": "monitoring",
  "settings-alerts": "alerts",
  "settings-log-rules": "alerts",
  "settings-deps": "alerts",
  "settings-notifications": "alerts",
  "settings-notify-routes": "alerts",
  "settings-ntfy": "alerts",
  "settings-desktop": "general",
  "settings-updates": "general",
  "settings-transmissions": "general",
  "settings-releases": "general",
  "settings-diagnostics": "general",
  "settings-diagnostics-export": "general",
  "settings-sync": "sync",
  "sync-pair": "sync",
  "sync-join": "sync",
  "sync-peers": "sync",
  "sync-mode": "sync",
  "settings-backup": "data",
  "dashboard-export": "data",
  "settings-status-page": "data",
  "settings-tailnet": "connections",
  "tailnet-login": "connections",
  "settings-keys": "connections",
  "settings-mcp": "connections",
  "mcp-config": "connections",
  "mcp-ask": "connections",
  "keys-new": "connections",
  "keys-check": "connections",
  "keys-matrix": "connections",
  "keys-rotate": "connections",
  "settings-helper": "drift",
  "settings-models": "drift",
  "settings-tours": "drift",
  "settings-features": "features",
  "settings-feature-presets": "features",
};

/** The tab a settings target lives on (null: not a settings target or a tab button itself). */
export function settingsTabFor(target: string): SettingsTab | null {
  const tab = /^settings-tab-([a-z]+)$/.exec(target);
  if (tab) return null;
  if (target.startsWith("settings-feature")) return "features";
  return BY_TARGET[target] ?? null;
}

export const isSettingsTab = (x: string): x is SettingsTab => SETTINGS_TABS.some((t) => t.id === x);
