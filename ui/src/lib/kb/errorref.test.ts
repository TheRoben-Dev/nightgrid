import { describe, expect, it } from "vitest";
import type { Check } from "../helper/skills";
import { chunkMarkdown } from "../helper/knowledge";
import { ARTICLES } from "./library";
import { addErrorRefs, compile, entries, matchErrors, norm, parseEntries } from "./errorref";

describe("error reference", () => {
  it("parses every entry with a meaning and a fix, once", () => {
    const es = entries();
    expect(es.length).toBeGreaterThan(900);
    const raw = ARTICLES.filter((a) => a.shelf === "errors").reduce((n, a) => n + (a.body.match(/^### /gm) ?? []).length, 0);
    expect(es.length, "every ### entry needs Means: and Fix: lines").toBe(raw);
    const seen = new Map<string, string>();
    for (const e of es) {
      const k = norm(e.message);
      expect(seen.has(k), `${e.message} in ${e.articleId} and ${seen.get(k)}`).toBe(false);
      seen.set(k, e.articleId);
    }
  });

  it("keeps each entry small enough to be one retrieval chunk", () => {
    for (const a of ARTICLES.filter((x) => x.shelf === "errors")) {
      for (const c of chunkMarkdown(`## ${a.title}\n${a.body}`)) expect(c.text.length, `${a.id} › ${c.title}`).toBeLessThan(900);
    }
  });

  it("matches pasted messages, placeholders and all", () => {
    const top = (t: string) => matchErrors(t)[0]?.message;
    expect(top("bash: ./run.sh: /bin/bash^M: bad interpreter: No such file or directory")).toBe("/bin/bash^M: bad interpreter: No such file or directory");
    expect(top("Nov 17 03:12:01 srv kernel: Out of memory: Killed process 4242 (java) total-vm:812345kB")).toBe("Out of memory: Killed process <pid> (<name>)");
    expect(top("Job for nginx.service failed because the control process exited with error code.")).toBe("Job for <unit> failed because the control process exited with error code.");
    expect(top("umount: /mnt/data: target is busy.")).toBe("umount: <dir>: target is busy.");
    // Curly quotes and doubled spaces from a chat or web page still match.
    expect(top("syntax error near  unexpected token ‘fi’")).toBe("syntax error near unexpected token `<token>'");
  });

  it("prefers the specific message over a generic one inside it", () => {
    const hits = matchErrors("swapon: /swapfile: insecure permissions 0644, 0600 suggested.");
    expect(hits[0].message).toBe("swapon: <file>: insecure permissions 0644, 0600 suggested.");
  });

  it("ignores short or unrelated text", () => {
    expect(matchErrors("hi")).toEqual([]);
    expect(matchErrors("how do I add a host to nightgrid?")).toEqual([]);
    expect(compile("Killed")).toBeNull();
  });

  it("adds hits to the error check, keeping it short", () => {
    const checks: Check[] = [{ kind: "errors", title: "Error reference", items: [{ text: "a" }, { text: "b" }, { text: "c" }] }];
    addErrorRefs(checks, parseEntries("errors/x", "X", "### Some long error message here\nMeans: m\nFix: f"));
    expect(checks[0].items.map((i) => i.text)).toEqual(["a", "b", "Some long error message here (X): m Fix: f"]);
    const none: Check[] = [];
    addErrorRefs(none, []);
    expect(none).toEqual([]);
  });
});
