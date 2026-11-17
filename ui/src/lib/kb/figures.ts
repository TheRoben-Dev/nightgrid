/**
 * Library figures: SVGs bundled from docs/kb/figures, shown inside articles
 * as `![caption](fig:name)`. Drawn for Nightgrid (quest-log intel cards), no
 * scripts, no outside references; a test checks every file. Vite turns each
 * into a local asset URL (small ones inline as data: URIs), so nothing is
 * fetched from the network.
 */
const FILES = import.meta.glob("../../../../docs/kb/figures/*.svg", { query: "?url", import: "default", eager: true }) as Record<string, string>;

const BY_NAME = new Map(Object.entries(FILES).map(([p, url]) => [p.replace(/^.*\/([\w-]+)\.svg$/, "$1"), url]));

/** The bundled URL of a figure, or undefined for an unknown name. */
export const figure = (name: string): string | undefined => BY_NAME.get(name);
export const FIGURE_NAMES = [...BY_NAME.keys()];
