import { describe, expect, it } from "vitest";
import { cidrContains, cidrInfo, normMac } from "./cidr";

describe("subnet calculator", () => {
  it("works out a /24 from any address in it", () => {
    expect(cidrInfo("192.168.1.77/24")).toEqual({
      cidr: "192.168.1.0/24",
      network: "192.168.1.0",
      broadcast: "192.168.1.255",
      mask: "255.255.255.0",
      wildcard: "0.0.0.255",
      first: "192.168.1.1",
      last: "192.168.1.254",
      hosts: 254,
      scope: "private",
    });
  });
  it("takes dotted masks, /31, /32 and bare addresses", () => {
    expect(cidrInfo("10.20.0.5 255.255.0.0")?.cidr).toBe("10.20.0.0/16");
    expect(cidrInfo("10.0.0.0 255.0.255.0")).toBeNull();
    expect(cidrInfo("10.0.0.4/31")?.hosts).toBe(2);
    expect(cidrInfo("100.101.7.12")).toMatchObject({ cidr: "100.101.7.12/32", hosts: 1, scope: "CGNAT / tailnet" });
    expect(cidrInfo("8.8.8.8/8")?.scope).toBe("public");
    expect(cidrInfo("300.1.1.1/24")).toBeNull();
    expect(cidrInfo("1.2.3.4/33")).toBeNull();
    expect(cidrContains("192.168.1.0/24", "192.168.1.200")).toBe(true);
    expect(cidrContains("192.168.1.0/24", "192.168.2.1")).toBe(false);
  });
  it("normalises MAC addresses", () => {
    expect(normMac("AA-BB-CC-DD-EE-FF")).toBe("aa:bb:cc:dd:ee:ff");
    expect(normMac("aabb.ccdd.eeff")).toBe("aa:bb:cc:dd:ee:ff");
    expect(normMac("aa:bb:cc:dd:ee")).toBeNull();
    expect(normMac("zz:bb:cc:dd:ee:ff")).toBeNull();
  });
});
