import { describe, expect, it } from "vitest";
import type { PostureCheck, PostureHost } from "./api";
import { cost, delta, gradeTone, ordered, sparkline, summary, worstFirst } from "./posture";
import { article } from "./kb/library";

const c = (id: string, status: PostureCheck["status"], weight: number, art = "howto/firewall"): PostureCheck => ({ id, title: id, status, detail: "", fix: null, weight, article: art });
const host = (id: number, score: number | null, prev: number | null, checks: PostureCheck[] = []): PostureHost => ({
  latest: { hostId: id, at: 0, score, grade: score == null ? null : score >= 90 ? "A" : "C", checks, openPorts: 0, error: score == null ? "x" : null },
  previous: prev,
  trend: [],
});

describe("posture", () => {
  it("orders fixes and counts their cost", () => {
    const list = ordered([c("a", "pass", 20), c("b", "warn", 15), c("d", "fail", 5), c("e", "fail", 20)]);
    expect(list.map((x) => x.id)).toEqual(["e", "d", "b", "a"]);
    expect(cost(c("b", "warn", 15))).toBe(7);
    expect(cost(c("b", "fail", 15))).toBe(15);
  });
  it("summarises the fleet", () => {
    const hs = [host(1, 95, 90, [c("x", "pass", 5)]), host(2, 60, 80, [c("y", "fail", 20)]), host(3, null, null)];
    expect(summary(hs)).toEqual({ hosts: 3, average: 78, below: 1, failing: 1 });
    expect(worstFirst(hs).map((h) => h.latest.hostId)).toEqual([2, 1, 3]);
    expect(delta(hs[0])).toBe("+5");
    expect(delta(hs[1])).toBe("−20");
    expect(gradeTone("B")).toBe("ok");
    expect(gradeTone("D")).toBe("bad");
    expect(sparkline([100])).toBe("");
    expect(sparkline([100, 0], 10, 10)).toBe("0.0,0.0 10.0,10.0");
  });
  it("links to Library articles that exist", () => {
    for (const a of ["basics/sshd-hardening", "howto/firewall", "apps/fail2ban-crowdsec", "apps/unattended-upgrades", "howto/access-audit", "howto/fleet-updates", "guides/security-posture"]) {
      expect(article(a), a).toBeTruthy();
    }
  });
});
