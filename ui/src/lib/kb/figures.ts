/**
 * Library figures: SVGs bundled from docs/kb/figures, shown inside articles
 * as `![caption](fig:name)`. Drawn for Nightgrid (quest-log intel cards), no
 * scripts, no outside references; a test checks every file. Vite turns each
 * into a local asset URL (small ones inline as data: URIs), so nothing is
 * fetched from the network. A content pack (D-174) can add or replace
 * figures; those become data: URIs after the same safety check the tests
 * apply to the bundled files.
 */
const FILES = import.meta.glob("../../../../docs/kb/figures/*.svg", { query: "?url", import: "default", eager: true }) as Record<string, string>;

const BUNDLED = new Map(Object.entries(FILES).map(([p, url]) => [p.replace(/^.*\/([\w-]+)\.svg$/, "$1"), url]));
const BY_NAME = new Map(BUNDLED);

/** The URL of a figure, or undefined for an unknown name. */
export const figure = (name: string): string | undefined => BY_NAME.get(name);
export const FIGURE_NAMES = [...BY_NAME.keys()];

/** Nothing active and nothing fetched: no scripts, handlers, outside references or imports. */
export function svgSafe(svg: string): boolean {
  return /^<svg[\s>]/.test(svg) && /viewBox=/.test(svg) && svg.length < 200 * 1024 && !/<script|<foreignObject|\son[a-z]+\s*=|javascript:|(?:href|src)\s*=\s*["'](?!#)|url\((?!#)|@import/i.test(svg);
}

/** Adds a content pack's figures (keys like `figures/x.svg`), or goes back to the bundled set with null. */
export function applyFigures(files: Record<string, string> | null): void {
  BY_NAME.clear();
  for (const [k, v] of BUNDLED) BY_NAME.set(k, v);
  for (const [k, svg] of Object.entries(files ?? {})) {
    const m = /^figures\/([a-z0-9-]+)\.svg$/.exec(k);
    if (m && svgSafe(svg)) BY_NAME.set(m[1], `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`);
  }
}
