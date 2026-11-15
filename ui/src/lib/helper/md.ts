/**
 * A tiny markdown subset for DRIFT's replies: paragraphs, bullet and
 * numbered lists, fenced code blocks, inline code, bold and italics. It produces a
 * structure the component renders as text nodes, so model output can never
 * become HTML.
 */

export type Inline = { kind: "text" | "code" | "bold" | "em"; text: string };
export type Block =
  | { kind: "p"; lines: Inline[][] }
  | { kind: "ul" | "ol"; items: Inline[][]; start?: number }
  | { kind: "code"; lang: string; text: string; open?: boolean }
  | { kind: "h"; level: number; text: Inline[] };

export interface MdOptions {
  /** Keep `#` headings as heading blocks (notes); DRIFT folds them into text. */
  headings?: boolean;
}

export function inline(s: string): Inline[] {
  const out: Inline[] = [];
  const re = /`([^`]+)`|\*\*([^*]+)\*\*|(?<![\w*])\*([^*\s][^*\n]*?)\*(?![\w*])/g;
  let last = 0;
  for (let m = re.exec(s); m; m = re.exec(s)) {
    if (m.index > last) out.push({ kind: "text", text: s.slice(last, m.index) });
    out.push(m[1] !== undefined ? { kind: "code", text: m[1] } : m[2] !== undefined ? { kind: "bold", text: m[2] } : { kind: "em", text: m[3] });
    last = m.index + m[0].length;
  }
  if (last < s.length) out.push({ kind: "text", text: s.slice(last) });
  return out;
}

export function parseMd(src: string, opts: MdOptions = {}): Block[] {
  const heading = /^\s{0,3}(#{1,4})\s+(.*?)\s*#*\s*$/;
  const blocks: Block[] = [];
  const lines = src.replace(/\r\n/g, "\n").split("\n");
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const fence = /^\s*```\s*([\w+-]*)\s*$/.exec(line);
    if (fence) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !/^\s*```\s*$/.test(lines[i])) body.push(lines[i++]);
      const open = i >= lines.length;
      i++;
      blocks.push({ kind: "code", lang: fence[1], text: body.join("\n"), open });
      continue;
    }
    if (!line.trim()) {
      i++;
      continue;
    }
    const h = opts.headings ? heading.exec(line) : null;
    if (h) {
      blocks.push({ kind: "h", level: h[1].length, text: inline(h[2]) });
      i++;
      continue;
    }
    const bullet = /^\s*[-*•]\s+(.*)$/;
    const num = /^\s*\d+[.)]\s+(.*)$/;
    if (bullet.test(line) || num.test(line)) {
      const re = bullet.test(line) ? bullet : num;
      const start = re === num ? parseInt(line, 10) : undefined;
      const items: Inline[][] = [];
      while (i < lines.length && re.test(lines[i])) items.push(inline(re.exec(lines[i++])![1]));
      blocks.push(re === bullet ? { kind: "ul", items } : { kind: "ol", items, start });
      continue;
    }
    const para: Inline[][] = [];
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^\s*```/.test(lines[i]) &&
      !bullet.test(lines[i]) &&
      !num.test(lines[i]) &&
      !(opts.headings && heading.test(lines[i]))
    ) {
      para.push(inline(lines[i++].replace(/^#+\s+/, "")));
    }
    blocks.push({ kind: "p", lines: para });
  }
  return blocks;
}
