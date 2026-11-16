import { describe, expect, it } from "vitest";
import { GUIDE_TARGETS } from "./guide";
import { chunkMarkdown, guideIndex, Index, terms } from "./knowledge";
import { inline, parseMd } from "./md";
import { buildTurns, contextBlock, MAX_SELECTION, parseReply, redact, streamingText, systemPrompt } from "./prompt";

const markup = Object.values(import.meta.glob("../../**/*.svelte", { query: "?raw", import: "default", eager: true }) as Record<string, string>).join("\n");

describe("guide targets", () => {
  it("every target exists in the markup", () => {
    for (const t of GUIDE_TARGETS) {
      const literal = markup.includes(`guide="${t.id}"`);
      const rail = t.id.startsWith("rail-") && markup.includes('data-guide="rail-{r.members[0]}"');
      const sysTab = t.id.startsWith("system-tab-") && markup.includes('data-guide="system-tab-{t.id}"');
      const fleetTab = t.id.startsWith("fleet-tab-") && markup.includes('data-guide="fleet-tab-{t.id}"');
      const pveTab = t.id.startsWith("proxmox-tab-") && markup.includes('data-guide="proxmox-tab-{t.id}"');
      const viewTab = t.id.startsWith("viewtab-") && markup.includes('data-guide="viewtab-{v.id}"');
      expect(literal || rail || sysTab || fleetTab || pveTab || viewTab, t.id).toBe(true);
    }
  });
  it("views that share a sidebar row point at the row and say where they are", () => {
    expect(GUIDE_TARGETS.find((t) => t.id === "rail-fleet")?.what).toContain("System → Fleet");
    expect(GUIDE_TARGETS.find((t) => t.id === "viewtab-tunnels")?.view).toBe("network");
    expect(GUIDE_TARGETS.find((t) => t.id === "rail-settings")?.what).toContain("bottom");
  });
  it("ids are unique", () => {
    expect(new Set(GUIDE_TARGETS.map((t) => t.id)).size).toBe(GUIDE_TARGETS.length);
  });
});

describe("knowledge", () => {
  it("chunks by heading and keeps chunks small", () => {
    const c = chunkMarkdown("# T\nintro\n\n## Hosts\n- a\n- b\n\n## Files\ntext " + "x ".repeat(600) + "\n\nmore");
    expect(c.map((x) => x.title)).toEqual(["Nightgrid", "Hosts", "Files", "Files"]);
    expect(c.every((x) => x.text.length <= 1300)).toBe(true);
  });
  it("stems and drops stop words", () => {
    expect(terms("How do I add hosts?")).toEqual(["add", "host"]);
  });
  it("finds the right guide section", () => {
    const idx = guideIndex();
    expect(idx.search("how do I export an encrypted backup")[0].title).toMatch(/Backups/);
    expect(idx.search("ssh tunnel socks proxy")[0].title).toBe("Tunnels");
    expect(idx.search("steam deck controller")[0].title).toMatch(/Handhelds/);
    expect(idx.search("zzzz qqqq")).toEqual([]);
  });
  it("ranks a title match above a passing mention", () => {
    const idx = new Index([
      { title: "Docker", text: "containers and stacks" },
      { title: "Other", text: "mentions docker once among many other words here" },
    ]);
    expect(idx.search("docker")[0].title).toBe("Docker");
  });
});

describe("redact", () => {
  it("masks secret-looking values", () => {
    const r = redact(
      "DB_PASSWORD=hunter2\nAPI_KEY: abc123\nexport TOKEN='x y'\nAuthorization: Bearer abcdefghijklmnop1234\nurl postgres://app:s3cret@db:5432/x\nPORT=8080",
    );
    expect(r).not.toMatch(/hunter2|abc123|x y|abcdefghijklmnop1234|s3cret/);
    expect(r).toContain("PORT=8080");
    expect(r).toContain("postgres://app:••••@db");
    expect(redact("-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n-----END OPENSSH PRIVATE KEY-----")).toBe("[private key removed]");
  });
});

describe("prompt", () => {
  it("lists views and targets in the system prompt", () => {
    const s = systemPrompt();
    expect(s).toContain("docker (Docker)");
    expect(s).toContain("hosts-add:");
    expect(s).toContain("fleet (System → Fleet)");
    expect(s).toContain("System (tabs: One host, Fleet)");
    expect(s).toContain("Knowledge");
    expect(s).toContain("Settings is at the bottom");
  });
  it("puts context and guide only on the newest question", () => {
    const docs = [{ title: "Hosts", text: "Use + to add." }];
    const t = buildTurns(
      [
        { role: "user", content: "hi" },
        { role: "assistant", content: "hello" },
      ],
      "how to add a host? my password=abc",
      { view: "docker", host: "nas", selection: "error: boom" },
      docs,
    );
    expect(t.map((x) => x.role)).toEqual(["system", "user", "assistant", "user"]);
    const last = t[3].content;
    expect(last).toContain('Current view: Docker, host "nas"');
    expect(last).toContain("## Hosts\nUse + to add.");
    expect(last).toContain("error: boom");
    expect(last).not.toContain("abc");
  });
  it("labels text a view attached", () => {
    const b = contextBlock({ view: "system", selection: "Failed to start nginx", source: "journal: nginx.service" }, []);
    expect(b).toContain("Attached (journal: nginx.service):\n```\nFailed to start nginx\n```");
    expect(contextBlock({ view: "system", selection: "x" }, [])).toContain("Selected text:");
  });
  it("cuts long selections from the start", () => {
    const b = contextBlock({ view: "overview", selection: "a".repeat(MAX_SELECTION) + "TAIL" }, []);
    expect(b).toContain("TAIL");
    expect(b).toContain("[earlier lines cut]");
  });
});

describe("parseReply", () => {
  it("extracts valid actions and strips them from the text", () => {
    const r = parseReply("Open Settings.\n\n[[show settings]]\n[[point settings-backup: Export is here]]\n[[point nope: x]]\n[[show nowhere]]");
    expect(r.text).toBe("Open Settings.");
    expect(r.actions).toEqual([
      { kind: "show", view: "settings" },
      { kind: "point", id: "settings-backup", note: "Export is here" },
    ]);
  });
  it("limits actions and leaves normal brackets alone", () => {
    const r = parseReply("Use `[[x]]` in text\n[[show docker]]\n[[show files]]\n[[point docker-stacks]]");
    expect(r.text).toBe("Use `[[x]]` in text");
    expect(r.actions).toEqual([
      { kind: "show", view: "docker" },
      { kind: "point", id: "docker-stacks", note: "" },
    ]);
  });
  it("hides half-streamed action lines", () => {
    expect(streamingText("Hello\n[[poi")).toBe("Hello");
  });
});

describe("md", () => {
  it("parses the subset and never keeps HTML as markup", () => {
    const b = parseMd("Hi **there** `x`\n\n- one\n- two\n\n1. a\n2. b\n\n```sh\nls -la\n```\n<b>raw</b>");
    expect(b.map((x) => x.kind)).toEqual(["p", "ul", "ol", "code", "p"]);
    expect(inline("a **b** `c`")).toEqual([
      { kind: "text", text: "a " },
      { kind: "bold", text: "b" },
      { kind: "text", text: " " },
      { kind: "code", text: "c" },
    ]);
    expect(b[3]).toEqual({ kind: "code", lang: "sh", text: "ls -la", open: false });
    expect(b[4]).toEqual({ kind: "p", lines: [[{ kind: "text", text: "<b>raw</b>" }]] });
  });
  it("supports italics without eating bullets or maths", () => {
    expect(inline("tick *Include saved* now")[1]).toEqual({ kind: "em", text: "Include saved" });
    expect(inline("2 * 3 * 4")).toEqual([{ kind: "text", text: "2 * 3 * 4" }]);
  });
  it("handles an unterminated fence while streaming", () => {
    expect(parseMd("```\nabc")).toEqual([{ kind: "code", lang: "", text: "abc", open: true }]);
  });
});

describe("notes in DRIFT", () => {
  it("indexes notes, skips hidden ones and lists the notes used", async () => {
    const { notesIndex, noteChunks, noteRefs } = await import("./knowledge");
    const ns = [
      { id: 1, title: "Replace a disk", body: "Run `zpool replace tank old new` and wait for resilver.", tags: ["zfs"], hideFromDrift: false, updatedAt: 1 },
      { id: 2, title: "Router login", body: "zpool notes kept private", tags: [], hideFromDrift: true, updatedAt: 1 },
    ];
    expect(noteChunks(ns).every((c) => c.noteId === 1)).toBe(true);
    const hits = notesIndex(ns).search("how do I replace a zpool disk", 2);
    expect(hits[0].noteId).toBe(1);
    expect(hits[0].title).toContain("Replace a disk");
    expect(noteRefs([...hits, { title: "Hosts" }])).toEqual([{ id: 1, title: "Replace a disk" }]);
  });
});

describe("runbook steps in notes", () => {
  it("turns task items into checks and view links into links, only when asked", () => {
    const src = "- [ ] Check `df -h`\n- [x] Prune images\n- plain\n\nOpen [disks](view:fleet) now";
    const b = parseMd(src, { headings: true, steps: true });
    expect(b[0]).toMatchObject({ kind: "ul", checks: [false, true, null] });
    expect(b[0].kind === "ul" && b[0].items[0]).toEqual([{ kind: "text", text: "Check " }, { kind: "code", text: "df -h" }]);
    expect(b[1].kind === "p" && b[1].lines[0]).toEqual([{ kind: "text", text: "Open " }, { kind: "view", text: "disks", view: "fleet" }, { kind: "text", text: " now" }]);
    const plain = parseMd(src);
    expect(plain[0]).not.toHaveProperty("checks");
    expect(plain[1].kind === "p" && plain[1].lines[0]).toEqual([{ kind: "text", text: "Open [disks](view:fleet) now" }]);
  });
});
