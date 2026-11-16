import { describe, expect, it } from "vitest";
import { ago, diffLines, repoName, shortSha, validBranch, validDir, validSubdir, validUrl } from "./gitstacks";
import { mockGitStacks } from "./mockGitStacks";

describe("git stack helpers", () => {
  it("validates like the backend", () => {
    for (const ok of ["https://github.com/a/b.git", "git@github.com:a/b.git", "ssh://git@host:2222/a/b", "/srv/git/b.git"]) expect(validUrl(ok)).toBe(true);
    for (const bad of ["", "-oProxyCommand=x", "https://a b", "relative/path"]) expect(validUrl(bad)).toBe(false);
    expect(validBranch("release/1.2")).toBe(true);
    for (const bad of ["", "-x", "a..b", "a/", "x.lock", "a b", "a;b"]) expect(validBranch(bad)).toBe(false);
    expect(validSubdir("") && validSubdir("stacks/media")).toBe(true);
    for (const bad of ["/abs", "a/../b", "a//b", "a/"]) expect(validSubdir(bad)).toBe(false);
    expect(validDir("~/stacks/x") && validDir("/srv/x")).toBe(true);
    expect(validDir("/") || validDir("rel")).toBe(false);
  });

  it("names and shortens", () => {
    expect(repoName("git@git.example.com:me/homelab.git")).toBe("homelab");
    expect(repoName("https://example.com/me/media/")).toBe("media");
    expect(shortSha("0123456789abcdef")).toBe("01234567");
    expect(ago(1000, 1000 + 7200)).toBe("2 hours ago");
    expect(ago(1000, 1030)).toBe("just now");
    expect(ago(null)).toBe("");
  });

  it("colours a unified diff", () => {
    const kinds = diffLines("diff --git a/x b/x\nindex 1..2 100644\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n same\n").map((l) => l.kind);
    expect(kinds).toEqual(["file", "meta", "meta", "meta", "hunk", "del", "add", "ctx"]);
    expect(diffLines("")).toEqual([]);
  });
});

describe("mock git stacks", () => {
  it("checks, deploys and rolls back", async () => {
    const [g] = await mockGitStacks.list(1);
    const s = await mockGitStacks.check(g.id);
    expect(s.behind).toBe(2);
    const d = await mockGitStacks.deploy(g.id, s.remote!);
    expect(d.git.previous).toBe(g.deployed);
    expect((await mockGitStacks.check(g.id)).behind).toBe(0);
    expect(await mockGitStacks.diff(g.id, d.git.previous!)).toContain("-    image: jellyfin/jellyfin:10.10.3");
    await expect(mockGitStacks.add(1, { url: "nope", branch: "main", dir: "~/x" })).rejects.toThrow(/url/);
  });
});
