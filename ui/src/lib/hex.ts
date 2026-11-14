/** Deterministic decorative hex IDs (`0x1F4A`) from a seed string. Decoration only. */
export function hexId(seed: string, digits = 4): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return "0x" + h.toString(16).toUpperCase().padStart(8, "0").slice(0, digits);
}

/** Deterministic bar widths for the decorative barcode strip. */
export function barcode(seed: string, count = 48): number[] {
  const out: number[] = [];
  let h = parseInt(hexId(seed, 8).slice(2), 16) || 1;
  for (let i = 0; i < count; i++) {
    h ^= h << 13;
    h ^= h >>> 17;
    h ^= h << 5;
    h >>>= 0;
    out.push(1 + (h % 4));
  }
  return out;
}
