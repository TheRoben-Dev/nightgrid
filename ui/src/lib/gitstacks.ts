/** Pure helpers for stacks from git (M10w). Validation mirrors core/src/docker/gitstack.rs. */

export const shortSha = (c: string | null | undefined): string => (c ? c.slice(0, 8) : "");

export function validUrl(u: string): boolean {
  if (!u || u.length > 2048 || u.startsWith("-") || /\s/.test(u) || [...u].some((c) => c.charCodeAt(0) < 32)) return false;
  if (/^(https?|ssh|git|file):\/\//.test(u)) return true;
  if (u.startsWith("/")) return true;
  const colon = u.indexOf(":");
  const slash = u.indexOf("/");
  return colon > 0 && (slash < 0 || colon < slash) && !u.includes("://");
}

export function validBranch(b: string): boolean {
  return (
    b.length > 0 &&
    b.length <= 200 &&
    !/^[-/.]/.test(b) &&
    !/[/.]$/.test(b) &&
    !b.endsWith(".lock") &&
    !b.includes("..") &&
    !b.includes("//") &&
    !b.includes("@{") &&
    /^[A-Za-z0-9._\-/+]+$/.test(b)
  );
}

export function validSubdir(s: string): boolean {
  if (!s) return true;
  return s.length <= 512 && !s.startsWith("/") && !s.endsWith("/") && s.split("/").every((p) => p !== "" && p !== "." && p !== "..");
}

export const validDir = (d: string): boolean => (d.startsWith("/") || d.startsWith("~/")) && d !== "/" && d.length <= 1024;

/** The repository's name from its URL: `git@x:me/media.git` → `media`. */
export function repoName(url: string): string {
  const last = url.replace(/\/+$/, "").split(/[/:]/).pop() ?? "";
  return last.replace(/\.git$/, "");
}

export type DiffKind = "file" | "hunk" | "add" | "del" | "ctx" | "meta";

/** Splits a unified diff into lines to colour. */
export function diffLines(text: string): { kind: DiffKind; text: string }[] {
  const out: { kind: DiffKind; text: string }[] = [];
  if (!text.trim()) return out;
  for (const line of text.replace(/\n$/, "").split("\n")) {
    let kind: DiffKind = "ctx";
    if (line.startsWith("diff --git ")) kind = "file";
    else if (line.startsWith("@@")) kind = "hunk";
    else if (line.startsWith("+++ ") || line.startsWith("--- ") || /^(index |new file|deleted file|similarity|rename |old mode|new mode|Binary )/.test(line)) kind = "meta";
    else if (line.startsWith("+")) kind = "add";
    else if (line.startsWith("-")) kind = "del";
    out.push({ kind, text: line });
  }
  return out;
}

/** "3 days ago", "just now". */
export function ago(ts: number | null | undefined, now = Date.now() / 1000): string {
  if (!ts) return "";
  const s = Math.max(0, now - ts);
  const units: [number, string][] = [
    [86400 * 365, "year"],
    [86400 * 30, "month"],
    [86400, "day"],
    [3600, "hour"],
    [60, "minute"],
  ];
  for (const [n, w] of units) {
    if (s >= n) {
      const v = Math.floor(s / n);
      return `${v} ${w}${v === 1 ? "" : "s"} ago`;
    }
  }
  return "just now";
}
