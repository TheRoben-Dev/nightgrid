import { describe, expect, it } from "vitest";
import { blankQuery, highlight, hitsText, sourceCounts, splitSource } from "./logsearch";

describe("logsearch", () => {
  it("highlights plain text case-insensitively and escapes it", () => {
    const q = { ...blankQuery(), text: "a.b" };
    expect(highlight("xA.Bx a-b", q)).toEqual([
      { t: "x", hit: false },
      { t: "A.B", hit: true },
      { t: "x a-b", hit: false },
    ]);
    expect(highlight("abc", { ...q, text: "" })).toEqual([{ t: "abc", hit: false }]);
  });

  it("highlights regex matches and survives bad patterns", () => {
    const q = { ...blankQuery(), text: "5\\d\\d", regex: true };
    expect(highlight("got 502 then 503", q).filter((p) => p.hit).map((p) => p.t)).toEqual(["502", "503"]);
    expect(highlight("x", { ...q, text: "(" })).toEqual([{ t: "x", hit: false }]);
  });

  it("splits and counts sources", () => {
    expect(splitSource("container/web")).toEqual({ kind: "container", name: "web" });
    expect(splitSource("journal/sshd")).toEqual({ kind: "journal", name: "sshd" });
    const h = (source: string) => ({ hostId: 1, source, ts: 0, text: "x" });
    expect(sourceCounts([h("journal/sshd"), h("container/web"), h("journal/sshd")])).toEqual([
      { source: "journal/sshd", n: 2 },
      { source: "container/web", n: 1 },
    ]);
    expect(hitsText([h("journal/sshd")], () => "server01")).toBe("1970-01-01T00:00:00.000Z server01 journal/sshd: x");
  });
});
