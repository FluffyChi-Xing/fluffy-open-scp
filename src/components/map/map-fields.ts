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
 *
 * ED is sampled at 16 m per texel.  A road can therefore be several texels
 * wide, especially after the game has rasterised a junction.  The old
 * three-texel probe rejected those roads wholesale, leaving only the dirt
 * colour baked into the terrain.  Probe farther out and keep the boundary
 * cells; a completely dry/empty field still has no boundary and is rejected.
 * This is not a substitute for the game's EcoPathSet or bridge classification.
 */
export function roadPreviewMask(eco: Uint8ClampedArray, size: number): Uint8Array {
  const candidates = new Uint8Array(size * size);
  for (let i = 0; i < candidates.length; i++) {
    candidates[i] = Number(eco[i * 4 + 1] === 0 && eco[i * 4 + 2] === 0);
  }
  const out = new Uint8Array(candidates.length);
  const seen = new Uint8Array(candidates.length);
  const stack: number[] = [];
  // Classify connected ED road-control regions.  A completely zero field is
  // one square component and is rejected; a rasterised road is a thin,
  // elongated component and its whole interior is retained so it renders as
  // a continuous strip instead of isolated boundary cells.
  for (let start = 0; start < candidates.length; start++) {
    if (!candidates[start] || seen[start]) continue;
    seen[start] = 1;
    stack.push(start);
    const cells: number[] = [];
    let minX = size, maxX = 0, minY = size, maxY = 0;
    while (stack.length) {
      const i = stack.pop()!;
      cells.push(i);
      const x = i % size, y = Math.floor(i / size);
      minX = Math.min(minX, x); maxX = Math.max(maxX, x);
      minY = Math.min(minY, y); maxY = Math.max(maxY, y);
      for (const n of [i - 1, i + 1, i - size, i + size]) {
        if (n < 0 || n >= candidates.length || seen[n] || !candidates[n]) continue;
        // Prevent left/right wrapping at row boundaries.
        if (Math.abs((n % size) - x) > 1) continue;
        seen[n] = 1;
        stack.push(n);
      }
    }
    const width = maxX - minX + 1, height = maxY - minY + 1;
    const elongated = Math.max(width, height) / Math.max(1, Math.min(width, height)) >= 1.6;
    const bounded = cells.length <= candidates.length * 0.35;
    if (elongated && bounded) for (const i of cells) out[i] = 1;
  }
  return out;
}

/** Stable world-cell seed keeps placements fixed when switching region crops. */
export function cellRandom(x: number, y: number, salt = 0): number {
  let h = Math.imul(x, 374761393) ^ Math.imul(y, 668265263) ^ salt;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}
