import { describe, expect, it } from "vitest";
import { ARTICLES, article, parts, stepChecks, takeChecks } from "./library";
import { questSummary } from "./quest";

describe("library quests", () => {
  it("takes check marks off step lines only", () => {
    const t = takeChecks("- [ ] one {check: swap}\n- [ ] two\n\n```sh\n- [ ] x {check: docker}\n```\n\nText {check: docker}\n1. [ ] three {check: port:443}");
    expect(t.checks).toEqual(["swap", null, "port:443"]);
    expect(t.body).toContain("- [ ] one\n");
    expect(t.body).toContain("- [ ] x {check: docker}");
    expect(t.body).toContain("Text {check: docker}");
  });

  it("lines checks up with the objective keys", () => {
    let withChecks = 0;
    for (const a of ARTICLES.filter((x) => x.checks)) {
      withChecks++;
      expect(a.checks!.length, a.id).toBe(parts(a).flatMap((p) => p.keys).length);
      expect(a.body, a.id).not.toContain("{check:");
    }
    expect(withChecks).toBeGreaterThan(10);
    const s = article("guides/starter-homelab")!;
    const map = stepChecks(s);
    expect(Object.values(map)).toContain("ssh-no-password");
    expect(Object.values(map)).toContain("container:caddy");
  });

  it("sums up a run", () => {
    expect(questSummary([{ id: "a", state: "pass", detail: "" }, { id: "b", state: "fail", detail: "" }, { id: "c", state: "unknown", detail: "" }])).toEqual({ pass: 1, fail: 1, unknown: 1, total: 3 });
  });
});
