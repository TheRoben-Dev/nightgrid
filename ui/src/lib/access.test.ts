import { describe, expect, it } from "vitest";
import { ago, bannedNow, keyName, limits, reportText, shortFp, worstTone } from "./access";
import { mockAccess } from "./mockAccess";

describe("access audit helpers", () => {
  it("summarises a report", async () => {
    const full = await mockAccess.audit(1, false);
    expect(worstTone(full.findings)).toBe("ok");
    expect(bannedNow(full)).toBe(1);
    expect(limits(full)).toEqual([]);
    const limited = await mockAccess.audit(2, false);
    expect(worstTone(limited.findings)).toBe("warn");
    expect(limits(limited)).toEqual(["logins and failed attempts", "fail2ban", "sudo rules", "other users' keys"]);
    expect(worstTone((await mockAccess.audit(2, true)).findings)).toBe("bad");
    expect(reportText(full, "server01")).toContain("failed 412x from 203.0.113.7");
  });

  it("names local keys and formats times", () => {
    const local = [{ fingerprint: "SHA256:aaa", file: "/Users/me/.ssh/id_ed25519", comment: "me" }];
    expect(keyName("SHA256:aaa", local)).toBe("id_ed25519");
    expect(keyName("SHA256:bbb", local)).toBeNull();
    expect(keyName(null, local)).toBeNull();
    expect(shortFp("SHA256:abcdefghijklmnop")).toBe("abcdefghijkl…");
    expect(ago(1000, 1030)).toBe("just now");
    expect(ago(0, 7200)).toBe("2 h ago");
    expect(ago(0, 5 * 86_400)).toBe("5 d ago");
  });
});
