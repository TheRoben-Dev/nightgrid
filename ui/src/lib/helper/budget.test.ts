import { describe, expect, it } from "vitest";
import { chunkMarkdown, guideIndex, MAX_CHUNK, splitBlock } from "./knowledge";
import { DEFAULT_CONTEXT, estimateTokens, fitTurns, MAX_DOC, REPLY_TOKENS, systemPrompt } from "./prompt";
import { GUIDE_TARGETS } from "./guide";
import { libraryIndex } from "../kb/library";

const budget = DEFAULT_CONTEXT - REPLY_TOKENS - 64;

describe("chunking", () => {
  it("splits long lists at top-level items, keeping nested lines", () => {
    const items = Array.from({ length: 30 }, (_, i) => `- item ${i} ${"word ".repeat(10)}\n  - nested ${i}`).join("\n");
    const parts = splitBlock(items);
    expect(parts.length).toBeGreaterThan(1);
    for (const p of parts) {
      expect(p.length).toBeLessThanOrEqual(MAX_CHUNK);
      expect(p.startsWith("- item")).toBe(true);
    }
    expect(parts.join("\n")).toBe(items);
  });
  it("keeps code fences closed and table headers on every piece", () => {
    const code = "```sh\n" + Array.from({ length: 80 }, (_, i) => `echo line ${i}`).join("\n") + "\n```";
    for (const p of splitBlock(code)) {
      expect(p.startsWith("```sh\n")).toBe(true);
      expect(p.endsWith("\n```")).toBe(true);
    }
    const table = "| a | b |\n|---|---|\n" + Array.from({ length: 80 }, (_, i) => `| row ${i} | value ${i} |`).join("\n");
    for (const p of splitBlock(table)) expect(p.startsWith("| a | b |\n|---|---|\n")).toBe(true);
  });
  it("cuts a single huge line", () => {
    for (const p of splitBlock("x".repeat(5000))) expect(p.length).toBeLessThanOrEqual(MAX_CHUNK);
  });
  it("leaves no oversized chunk in the guide or the library", () => {
    const docs = [...(guideIndex() as unknown as { docs: { chunk: { text: string } }[] }).docs, ...(libraryIndex() as unknown as { docs: { chunk: { text: string } }[] }).docs];
    expect(Math.max(...docs.map((d) => d.chunk.text.length))).toBeLessThanOrEqual(MAX_CHUNK);
    expect(chunkMarkdown("## T\n" + "- a\n".repeat(400)).every((c) => c.text.length <= MAX_CHUNK)).toBe(true);
  });
});

describe("system prompt", () => {
  it("lists only the targets the question is about", () => {
    const all = systemPrompt({ question: "", view: "overview" });
    const docker = systemPrompt({ question: "how do i set up docker?", view: "overview", titles: ["Docker"] });
    expect(docker).toContain("docker-stacks:");
    expect(docker).toContain("hosts-add:");
    expect(docker).not.toContain("keys-rotate:");
    expect(all).not.toContain("docker-stacks:");
    // Every sidebar row is always there.
    for (const t of GUIDE_TARGETS.filter((t) => t.id.startsWith("rail-"))) expect(all).toContain(`${t.id}:`);
    // Far from the 27k characters it took when every target was listed.
    expect(docker.length).toBeLessThan(12_000);
  });
});

describe("fitTurns", () => {
  const big = (title: string, n = 4000) => ({ title, text: `${title} ${"lorem ipsum ".repeat(n / 12)}` });
  it("fits the screenshot's first question in an 8k window", () => {
    const q = "how do i set up docker?";
    const docs = [...guideIndex().search(`${q} Overview`, 4), ...libraryIndex().search(q, 2), { title: "Note “Docker Quick Fixes”", text: "x ".repeat(2000), noteId: 1 }];
    const f = fitTurns([], q, { view: "overview" }, docs);
    expect(f.tokens).toBeLessThanOrEqual(budget);
    expect(f.docs.length).toBeGreaterThan(0);
  });
  it("cuts long sections and drops the lowest ranked first", () => {
    const docs = [big("G1"), big("G2"), big("G3"), big("G4"), { ...big("L1"), articleId: "a" }, { ...big("N1"), noteId: 1 }];
    const f = fitTurns([], "q", { view: "overview" }, docs, 4096);
    for (const d of f.docs) expect(d.text.length).toBeLessThanOrEqual(MAX_DOC);
    expect(f.tokens).toBeLessThanOrEqual(4096 - REPLY_TOKENS - 64);
    // The best of each kind outlasts the rest of the guide.
    expect(f.docs.map((d) => d.title)).toContain("G1");
    expect(f.trimmed).toContain("G4");
    expect(f.trimmed).not.toContain("G1");
  });
  it("drops older turns before the last sections, in question/answer pairs", () => {
    const history = Array.from({ length: 8 }, (_, i) => ({ role: (i % 2 ? "assistant" : "user") as "user" | "assistant", content: "y ".repeat(1500) }));
    const f = fitTurns(history, "q", { view: "overview" }, [big("G1"), big("G2")], DEFAULT_CONTEXT);
    expect(f.tokens).toBeLessThanOrEqual(budget);
    expect(f.trimmed).toContain("earlier messages");
    expect(f.turns[1].role === "user" || f.turns.length === 2).toBe(true);
  });
  it("shrinks a huge selection and huge live facts instead of failing", () => {
    const f = fitTurns([], "why?", { view: "docker", selection: "log line\n".repeat(4000), hosts: ["h ".repeat(5000)], checks: "c ".repeat(5000) }, [], DEFAULT_CONTEXT);
    expect(f.tokens).toBeLessThanOrEqual(budget);
    expect(estimateTokens(f.turns[f.turns.length - 1].content)).toBeGreaterThan(100);
    expect(f.turns[f.turns.length - 1].content).toContain("Question: why?");
  });
});
