/**
 * Backend access. Inside Tauri this calls the generated, typed commands; in a
 * plain browser (`pnpm dev` without Tauri, tests, screenshots) it falls back
 * to an in-memory/localStorage mock so the UI can be developed standalone.
 */
import { isTauri } from "@tauri-apps/api/core";
import { commands, type AppInfo_Serialize as AppInfo, type AppSettings_Serialize as AppSettings } from "./bindings";

export type { AppInfo, AppSettings };

export const DEFAULT_SETTINGS: AppSettings = {
  theme: "nightgrid",
  reduceEffects: false,
  scanlines: true,
  uiScale: 100,
  railCollapsed: false,
};

const MOCK_KEY = "nightgrid.mock.settings";

const mock = {
  async appInfo(): Promise<AppInfo> {
    return {
      version: "0.1.0-web",
      os: "browser",
      arch: "n/a",
      dataDir: "(browser)",
      logDir: "(browser)",
      schemaVersion: 1,
      keychain: { available: false, detail: "running outside Tauri" },
      dmabufDisabled: false,
    };
  },
  async settingsGet(): Promise<AppSettings> {
    try {
      const raw = localStorage.getItem(MOCK_KEY);
      return raw ? { ...DEFAULT_SETTINGS, ...JSON.parse(raw) } : { ...DEFAULT_SETTINGS };
    } catch {
      return { ...DEFAULT_SETTINGS };
    }
  },
  async settingsSet(s: AppSettings): Promise<AppSettings> {
    try {
      localStorage.setItem(MOCK_KEY, JSON.stringify(s));
    } catch {
      /* storage unavailable: keep in memory only */
    }
    return s;
  },
};

export const inTauri = (): boolean => {
  try {
    return isTauri();
  } catch {
    return false;
  }
};

export const api = {
  appInfo: (): Promise<AppInfo> => (inTauri() ? commands.appInfo() : mock.appInfo()),
  settingsGet: (): Promise<AppSettings> => (inTauri() ? commands.settingsGet() : mock.settingsGet()),
  settingsSet: (s: AppSettings): Promise<AppSettings> => (inTauri() ? commands.settingsSet(s) : mock.settingsSet(s)),
};
