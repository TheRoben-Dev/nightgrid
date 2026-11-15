/**
 * A tiny markdown subset for DRIFT's replies: paragraphs, bullet and
 * numbered lists, fenced code blocks, inline code, bold and italics. It produces a
 * structure the component renders as text nodes, so model output can never
 * become HTML.
 */

export type Inline = { kind: "text" | "code" | "bold" | "em"; text: string };
export type Block =
  | { kind: "p"; lines: Inline[][] }
  | { kind: "ul" | "ol"; items: Inline[][] }
  | { kind: "code"; lang: string; text: string; open?: boolean };

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

export function parseMd(src: string): Block[] {
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
    const bullet = /^\s*[-*•]\s+(.*)$/;
    const num = /^\s*\d+[.)]\s+(.*)$/;
    if (bullet.test(line) || num.test(line)) {
      const re = bullet.test(line) ? bullet : num;
      const items: Inline[][] = [];
      while (i < lines.length && re.test(lines[i])) items.push(inline(re.exec(lines[i++])![1]));
      blocks.push({ kind: re === bullet ? "ul" : "ol", items });
      continue;
    }
    const para: Inline[][] = [];
    while (i < lines.length && lines[i].trim() && !/^\s*```/.test(lines[i]) && !bullet.test(lines[i]) && !num.test(lines[i])) {
      para.push(inline(lines[i++].replace(/^#+\s+/, "")));
    }
    blocks.push({ kind: "p", lines: para });
  }
  return blocks;
}
