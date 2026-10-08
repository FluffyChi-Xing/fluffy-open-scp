/** Region raster coordinates are world XY; Three uses world XZ. */
export function fieldIndex(x: number, y: number, size: number, origin: readonly number[], spacing: number): number {
  const ix = Math.floor((x - origin[0]) / spacing);
  const iy = Math.floor((y - origin[1]) / spacing);
  return ix < 0 || iy < 0 || ix >= size || iy >= size ? -1 : iy * size + ix;
}

export function ecoChannel(kind: string | null | undefined): number | null {
  return ({ soil: 0, forest: 1, watertable: 2 } as Record<string, number>)[kind?.toLowerCase() ?? ""] ?? null;
}

/** Conservative preview mask. Exported road control clears both G and B.
 * Large dry patches cannot identify roads: reject interiors wider than 5 cells.
 * This is not a substitute for the game's EcoPathSet or bridge classification.
 */
export function roadPreviewMask(eco: Uint8ClampedArray, size: number): Uint8Array {
  const candidates = new Uint8Array(size * size);
  for (let i = 0; i < candidates.length; i++) {
    candidates[i] = Number(eco[i * 4 + 1] === 0 && eco[i * 4 + 2] === 0);
  }
  const out = new Uint8Array(candidates.length);
  for (let y = 3; y < size - 3; y++) for (let x = 3; x < size - 3; x++) {
    const i = y * size + x;
    if (!candidates[i]) continue;
    const narrowX = !candidates[i - 3] && !candidates[i + 3];
    const narrowY = !candidates[i - 3 * size] && !candidates[i + 3 * size];
    if (narrowX || narrowY) out[i] = 1;
  }
  return out;
}

/** Stable world-cell seed keeps placements fixed when switching region crops. */
export function cellRandom(x: number, y: number, salt = 0): number {
  let h = Math.imul(x, 374761393) ^ Math.imul(y, 668265263) ^ salt;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}
