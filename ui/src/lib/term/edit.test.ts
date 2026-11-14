import { describe, expect, it, vi } from "vitest";

// The session module pulls in xterm addons that need a canvas; only the parser is under test.
vi.mock("@xterm/addon-webgl", () => ({ WebglAddon: class {} }));

const { parseEditRequest } = await import("./session.svelte");

const token = "00112233445566778899aabbccddeeff";
const b64 = (s: string) => btoa(String.fromCharCode(...new TextEncoder().encode(s)));

describe("edit requests from the remote hook", () => {
  it("accepts our token with an absolute path", () => {
    expect(parseEditRequest(`${token};${b64("/srv/media/compose.yaml")}`, token)).toBe("/srv/media/compose.yaml");
    expect(parseEditRequest(`${token};${b64("/tmp/ünïcode file.txt")}`, token)).toBe("/tmp/ünïcode file.txt");
  });
  it("ignores other tokens, relative paths and junk", () => {
    expect(parseEditRequest(`deadbeef;${b64("/etc/passwd")}`, token)).toBeNull();
    expect(parseEditRequest(`${token};${b64("etc/passwd")}`, token)).toBeNull();
    expect(parseEditRequest(`${token};***`, token)).toBeNull();
    expect(parseEditRequest(token, token)).toBeNull();
  });
});
