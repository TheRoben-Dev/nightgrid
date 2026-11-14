/**
 * Strict-CSP support for libraries that inject `<style>` elements at runtime
 * (xterm.js: scrollbar, selection and DOM-renderer colours; Vite in dev).
 *
 * Our CSP has no `'unsafe-inline'` for styles, so such elements would be
 * blocked and reported as violations (browsers check CSP for any `<style>`,
 * whatever its `type`). Instead, `document.createElement("style")` returns an
 * inert `<nightgrid-style>` element (hidden by base.css), and its text is
 * mirrored into a constructable stylesheet in `document.adoptedStyleSheets`,
 * which CSP allows. Mirrors follow text changes and are dropped when the
 * element leaves the document. Libraries that need `HTMLStyleElement.sheet`
 * would not work with this; xterm.js and Vite only set `textContent`.
 */

export const MIRROR_TAG = "nightgrid-style";
const SELECTOR = MIRROR_TAG;
const sheets = new Map<HTMLElement, CSSStyleSheet>();
let installed = false;

const supported = (): boolean =>
  typeof document !== "undefined" && "adoptedStyleSheets" in document && typeof CSSStyleSheet !== "undefined";

const isMirrored = (n: unknown): n is HTMLElement => n instanceof HTMLElement && n.localName === MIRROR_TAG;

function sync(el: HTMLElement) {
  if (!el.isConnected) return;
  let sheet = sheets.get(el);
  if (!sheet) {
    sheet = new CSSStyleSheet();
    sheets.set(el, sheet);
    document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
  }
  try {
    sheet.replaceSync(el.textContent ?? "");
  } catch {
    /* malformed CSS: keep the previous rules */
  }
}

function prune() {
  for (const [el, sheet] of sheets) {
    if (!el.isConnected) {
      sheets.delete(el);
      document.adoptedStyleSheets = document.adoptedStyleSheets.filter((s) => s !== sheet);
    }
  }
}

function scan(n: Node) {
  if (isMirrored(n)) sync(n);
  else if (n instanceof Element) n.querySelectorAll<HTMLElement>(SELECTOR).forEach(sync);
}

/** Starts mirroring (idempotent). Call before any library creates styles. */
export function installCspStyleMirror() {
  if (installed || !supported()) return;
  installed = true;

  const create = document.createElement.bind(document);
  document.createElement = ((tag: string, options?: ElementCreationOptions) => {
    return create(tag.toLowerCase() === "style" ? MIRROR_TAG : tag, options);
  }) as typeof document.createElement;

  new MutationObserver((records) => {
    let removed = false;
    for (const r of records) {
      const owner = isMirrored(r.target) ? r.target : isMirrored(r.target.parentNode) ? r.target.parentNode : null;
      if (owner) sync(owner);
      r.addedNodes.forEach(scan);
      if (r.removedNodes.length) removed = true;
    }
    if (removed) prune();
  }).observe(document.documentElement, { subtree: true, childList: true, characterData: true });

  document.querySelectorAll<HTMLElement>(SELECTOR).forEach(sync);
}

/** Re-syncs mirrored styles under `root` (after re-attaching a detached terminal). */
export function resyncStyles(root: Element) {
  if (installed) scan(root);
}
