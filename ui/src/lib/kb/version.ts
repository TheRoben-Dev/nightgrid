/** Library content versions (D-174). */
declare const __KB_VERSION__: number | undefined;
/** The bundled content's version: the Unix time of its last commit (vite.config.ts). */
export const BUNDLED_KB: number = typeof __KB_VERSION__ === "number" ? __KB_VERSION__ : 0;

/** `2026-11-18 18:22` in local time, or "—" for an unknown version. */
export function kbVersionLabel(v: number): string {
  if (!v) return "—";
  const d = new Date(v * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}
