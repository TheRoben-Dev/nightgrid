import { describe, expect, it } from "vitest";
import { looksTailnet } from "./tailnetAddr";

describe("looksTailnet", () => {
  it("knows tailnet addresses and names", () => {
    expect(looksTailnet("100.101.12.3")).toBe(true);
    expect(looksTailnet("100.63.0.1")).toBe(false);
    expect(looksTailnet("100.128.0.1")).toBe(false);
    expect(looksTailnet("deck.tail1234.ts.net.")).toBe(true);
    expect(looksTailnet("[fd7a:115c:a1e0::5]")).toBe(true);
    expect(looksTailnet("192.168.1.20")).toBe(false);
    expect(looksTailnet(null)).toBe(false);
  });
});
