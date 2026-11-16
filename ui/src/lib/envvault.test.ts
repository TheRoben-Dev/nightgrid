import { describe, expect, it } from "vitest";
import { diffEmpty, diffSummary, looksSecret, splitPaste, validName } from "./envvault";
import { mockEnvVault } from "./mockEnvVault";

describe("env vault helpers", () => {
  it("checks names like the backend", () => {
    expect(validName("DB_PASSWORD")).toBe(true);
    expect(validName("_x1")).toBe(true);
    expect(validName("1ABC")).toBe(false);
    expect(validName("A-B")).toBe(false);
    expect(validName("")).toBe(false);
    expect(validName("A".repeat(129))).toBe(false);
  });

  it("masks secret-looking names", () => {
    expect(looksSecret("POSTGRES_PASSWORD")).toBe(true);
    expect(looksSecret("api_token")).toBe(true);
    expect(looksSecret("TZ")).toBe(false);
  });

  it("splits a pasted line", () => {
    expect(splitPaste("export FOO=bar baz")).toEqual({ name: "FOO", value: "bar baz" });
    expect(splitPaste('X="quoted"')).toEqual({ name: "X", value: "quoted" });
    expect(splitPaste("EMPTY=")).toEqual({ name: "EMPTY", value: "" });
    expect(splitPaste("no equals")).toBeNull();
  });

  it("summarizes a diff", () => {
    const d = { exists: true, added: ["A"], changed: [], removed: ["B", "C"], unchanged: 3 };
    expect(diffSummary(d)).toBe("1 added, 2 removed, 3 unchanged");
    expect(diffEmpty(d)).toBe(false);
    expect(diffEmpty({ exists: true, added: [], changed: [], removed: [], unchanged: 2 })).toBe(true);
    expect(diffEmpty({ exists: false, added: [], changed: [], removed: [], unchanged: 0 })).toBe(false);
  });
});

describe("mock vault", () => {
  const stack = { name: "app", dir: "/srv/app", files: [], running: 1, total: 1, inFolder: true };
  it("imports, edits and deploys", async () => {
    expect(await mockEnvVault.importHost(1, stack)).toBe(4);
    const vars = await mockEnvVault.list(1, "app");
    expect(vars.find((v) => v.name === "DB_PASSWORD")?.secret).toBe(true);
    expect(vars.find((v) => v.name === "TZ")?.secret).toBe(false);
    expect(await mockEnvVault.preview(1, stack)).toMatchObject({ added: [], changed: [], removed: [], unchanged: 4 });
    await mockEnvVault.set(1, "app", null, { name: "NEW", value: "1", secret: false });
    await mockEnvVault.remove(vars.find((v) => v.name === "PUID")!.id);
    expect(await mockEnvVault.preview(1, stack)).toMatchObject({ added: ["NEW"], removed: ["PUID"] });
    await expect(mockEnvVault.set(1, "app", "TZ", { name: "NEW", value: null, secret: false })).rejects.toThrow(/already/);
    await mockEnvVault.deploy(1, stack);
    expect(await mockEnvVault.preview(1, stack)).toMatchObject({ added: [], removed: [], unchanged: 4 });
  });
});
