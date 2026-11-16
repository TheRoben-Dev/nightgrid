import { describe, expect, it } from "vitest";
import { mockAuthNotices } from "../api";
import { authNotices } from "./authNotices.svelte";

describe("login notices", () => {
  it("shows a waiting host's approval link until hidden or connected", async () => {
    mockAuthNotices.push({ hostId: 4, notice: { target: "robin@100.101.1.2", text: "visit https://login.tailscale.com/a/x", url: "https://login.tailscale.com/a/x", tailscale: true } });
    await authNotices.poll();
    expect(authNotices.forHost(4)?.notice.url).toBe("https://login.tailscale.com/a/x");
    expect(authNotices.shown).toHaveLength(1);
    authNotices.dismiss(authNotices.shown[0]);
    expect(authNotices.shown).toHaveLength(0);
    mockAuthNotices.length = 0;
    await authNotices.poll();
    expect(authNotices.list).toHaveLength(0);
    expect(authNotices.dismissed).toHaveLength(0);
  });
});
