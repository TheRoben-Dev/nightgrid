import { describe, expect, it } from "vitest";
import { formatMode, formatSize, langFor, moveCursor, selectClick } from "./util";

describe("formatting", () => {
  it("sizes", () => {
    expect(formatSize(512)).toBe("512 B");
    expect(formatSize(1536)).toBe("1.5 KiB");
    expect(formatSize(2_754_981_888)).toBe("2.6 GiB");
    expect(formatSize(412_331_002)).toBe("393 MiB");
  });
  it("modes", () => {
    expect(formatMode(0o755, true)).toBe("drwxr-xr-x");
    expect(formatMode(0o640)).toBe("-rw-r-----");
    expect(formatMode(0o4755)).toBe("-rwsr-xr-x");
    expect(formatMode(0o1777, true)).toBe("drwxrwxrwt");
    expect(formatMode(0o2644)).toBe("-rw-r-Sr--");
    expect(formatMode(null)).toBe("—");
  });
});

describe("selection", () => {
  const order = ["a", "b", "c", "d", "e"];
  const none = { toggle: false, range: false };
  it("click selects one, ctrl toggles, shift ranges", () => {
    let s = selectClick(order, [], null, "b", none);
    expect(s).toEqual({ selected: ["b"], anchor: "b" });
    s = selectClick(order, s.selected, s.anchor, "d", { toggle: false, range: true });
    expect(s.selected).toEqual(["b", "c", "d"]);
    s = selectClick(order, s.selected, s.anchor, "c", { toggle: true, range: false });
    expect(s.selected).toEqual(["b", "d"]);
    s = selectClick(order, s.selected, "e", "a", { toggle: false, range: true });
    expect(s.selected).toEqual(["a", "b", "c", "d", "e"]);
  });
  it("cursor movement clamps", () => {
    expect(moveCursor(order, null, 1)).toBe("a");
    expect(moveCursor(order, null, -1)).toBe("e");
    expect(moveCursor(order, "e", 1)).toBe("e");
    expect(moveCursor(order, "b", -5)).toBe("a");
    expect(moveCursor([], "a", 1)).toBe(null);
  });
});

describe("language detection", () => {
  it.each([
    ["compose.yaml", "yaml"],
    ["docker-compose.yml", "yaml"],
    [".env", "properties"],
    ["prod.env", "properties"],
    ["Dockerfile", "dockerfile"],
    ["nginx.conf", "nginx"],
    ["app.conf", "properties"],
    ["backup.sh", "shell"],
    [".bashrc", "shell"],
    ["settings.json", "json"],
    ["config.toml", "toml"],
    ["README", "plain"],
  ])("%s → %s", (name, lang) => expect(langFor(name)).toBe(lang));
});
