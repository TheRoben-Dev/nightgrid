import { describe, expect, it } from "vitest";
import { GUIDE_TARGETS } from "./helper/guide";
import { SETTINGS_TABS, isSettingsTab, settingsTabFor } from "./settingsTabs";

describe("settings tabs", () => {
  it("maps every settings guide target to a tab", () => {
    const missing = GUIDE_TARGETS.filter((t) => t.view === "settings" && !t.id.startsWith("settings-tab-") && !settingsTabFor(t.id)).map((t) => t.id);
    expect(missing).toEqual([]);
  });

  it("has a guide target for every tab button", () => {
    for (const t of SETTINGS_TABS) expect(GUIDE_TARGETS.some((g) => g.id === `settings-tab-${t.id}`)).toBe(true);
  });

  it("puts controls where they belong", () => {
    expect(settingsTabFor("settings-log-rules")).toBe("alerts");
    expect(settingsTabFor("settings-backup")).toBe("data");
    expect(settingsTabFor("settings-feature-presets")).toBe("features");
    expect(settingsTabFor("settings-tab-drift")).toBeNull();
    expect(settingsTabFor("hosts-add")).toBeNull();
    expect(isSettingsTab("drift")).toBe(true);
    expect(isSettingsTab("nope")).toBe(false);
  });
});
