/**
 * Regex helper (M10p): explains a pattern piece by piece and tries it on
 * the attached lines, so DRIFT can say exactly what matches.
 */

export interface RegexFound {
  pattern: string;
  flags: string;
}

/** `/pattern/flags` or a backticked pattern after the word regex/pattern. */
export function findRegex(text: string): RegexFound | null {
  const slash = /(?:^|[\s`'"(])\/((?:\\.|\[(?:\\.|[^\]\\])*\]|[^/\\\n])+)\/([dgimsuy]*)(?=$|[\s`'".,)?!])/.exec(text);
  if (slash && /[\\^$.*+?()[\]{}|]/.test(slash[1])) return { pattern: slash[1], flags: slash[2] };
  const named = /\b(?:regex|regexp|pattern|grep(?:\s+-[EPio]+)*)\s*:?\s*`([^`\n]+)`/i.exec(text) ?? /`([^`\n]+)`\s+(?:regex|regexp|pattern)\b/i.exec(text);
  if (named) return { pattern: named[1], flags: "" };
  return null;
}

const ESC: Record<string, string> = {
  d: "a digit 0-9",
  D: "anything but a digit",
  w: "a word character (letter, digit or _)",
  W: "anything but a word character",
  s: "whitespace (space, tab, newline)",
  S: "anything but whitespace",
  b: "a word boundary",
  B: "not a word boundary",
  n: "a newline",
  t: "a tab",
  r: "a carriage return",
};

function quant(p: string, i: number): [string, number] {
  const c = p[i];
  let lazy = "";
  let txt: string;
  let len: number;
  if (c === "*") [txt, len] = ["0 or more times", 1];
  else if (c === "+") [txt, len] = ["1 or more times", 1];
  else if (c === "?") [txt, len] = ["optional (0 or 1 time)", 1];
  else if (c === "{") {
    const m = /^\{(\d+)(,(\d*))?\}/.exec(p.slice(i));
    if (!m) return ["", 0];
    len = m[0].length;
    txt = m[2] === undefined ? `exactly ${m[1]} times` : m[3] ? `${m[1]} to ${m[3]} times` : `${m[1]} or more times`;
  } else return ["", 0];
  if (p[i + len] === "?") {
    lazy = ", as few as possible";
    len++;
  }
  return [txt + lazy, len];
}

/** One line per piece of the pattern (top level; groups are described as a whole). */
export function explainRegex(p: string): string[] {
  const out: string[] = [];
  let i = 0;
  let lit = "";
  const flush = () => {
    if (lit) out.push(`"${lit}" literally`);
    lit = "";
  };
  while (i < p.length && out.length < 24) {
    const c = p[i];
    let piece: string;
    let len = 1;
    if (c === "\\") {
      const n = p[i + 1] ?? "";
      if (ESC[n]) piece = ESC[n];
      else if (/\d/.test(n)) piece = `the same text as group ${n}`;
      else {
        lit += n;
        i += 2;
        continue;
      }
      len = 2;
    } else if (c === "^") piece = "start of the line";
    else if (c === "$") piece = "end of the line";
    else if (c === ".") piece = "any character";
    else if (c === "|") piece = "OR";
    else if (c === "[") {
      let j = i + 1;
      if (p[j] === "^") j++;
      if (p[j] === "]") j++;
      while (j < p.length && p[j] !== "]") j += p[j] === "\\" ? 2 : 1;
      const set = p.slice(i, j + 1);
      piece = set.startsWith("[^") ? `any character except ${set.slice(2, -1)}` : `one of ${set.slice(1, -1)}`;
      len = set.length;
    } else if (c === "(") {
      let depth = 0;
      let j = i;
      for (; j < p.length; j++) {
        if (p[j] === "\\") {
          j++;
          continue;
        }
        if (p[j] === "(") depth++;
        else if (p[j] === ")" && --depth === 0) break;
      }
      const g = p.slice(i, j + 1);
      const inner = g.slice(1, -1);
      if (inner.startsWith("?:")) piece = `a group (not captured): ${inner.slice(2)}`;
      else if (inner.startsWith("?=")) piece = `followed by ${inner.slice(2)} (not included)`;
      else if (inner.startsWith("?!")) piece = `not followed by ${inner.slice(2)}`;
      else if (inner.startsWith("?<=")) piece = `preceded by ${inner.slice(3)}`;
      else if (inner.startsWith("?<!")) piece = `not preceded by ${inner.slice(3)}`;
      else if (/^\?<([A-Za-z]\w*)>/.test(inner)) piece = `a group named "${/^\?<([A-Za-z]\w*)>/.exec(inner)?.[1]}": ${inner.replace(/^\?<\w+>/, "")}`;
      else piece = `a captured group: ${inner}`;
      len = g.length;
    } else {
      const [q, ql] = quant(p, i + 1);
      if (q && lit) {
        // A quantifier applies to the last literal character only.
        const last = c;
        flush();
        out.push(`"${last}" ${q}`);
        i += 1 + ql;
        continue;
      }
      lit += c;
      i++;
      continue;
    }
    flush();
    const [q, ql] = quant(p, i + len);
    out.push(q ? `${piece}, ${q}` : piece);
    i += len + ql;
  }
  flush();
  return out;
}

export interface RegexTest {
  error?: string;
  lines: number;
  matched: number;
  samples: string[];
}

/** Tries the pattern on each line of `text`. */
export function testRegex(pattern: string, flags: string, text: string): RegexTest {
  let re: RegExp;
  try {
    re = new RegExp(pattern, flags.replace(/[gy]/g, ""));
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e), lines: 0, matched: 0, samples: [] };
  }
  const lines = text.split("\n").filter((l) => l.trim());
  const samples: string[] = [];
  let matched = 0;
  for (const l of lines) {
    const m = re.exec(l);
    if (!m) continue;
    matched++;
    if (samples.length < 5) {
      const groups = m.slice(1).filter((g) => g !== undefined);
      samples.push(`"${m[0].slice(0, 80)}"${groups.length ? ` (groups: ${groups.map((g) => `"${g.slice(0, 40)}"`).join(", ")})` : ""}`);
    }
  }
  return { lines: lines.length, matched, samples };
}

/** Differences that bite when a pattern moves to grep, sed or awk. */
export function toolNotes(pattern: string, question: string): string[] {
  const notes: string[] = [];
  const q = question;
  const shellTool = /\b(grep|sed|awk)\b/i.test(q);
  if (shellTool && /\\d/.test(pattern) && !/grep\s+-\w*P/.test(q)) notes.push("grep -E, sed and awk don't know \\d: use [0-9] (or grep -P).");
  if (shellTool && /\(\?[:=!<]/.test(pattern)) notes.push("Non-capturing groups and lookarounds need grep -P (PCRE); grep -E, sed and awk don't have them.");
  if (/\bgrep\b/i.test(q) && !/grep\s+-\w*[EP]/.test(q) && /[+?|(){}]/.test(pattern)) notes.push("Plain grep treats + ? | ( ) { } literally: use grep -E for this pattern.");
  return notes;
}
