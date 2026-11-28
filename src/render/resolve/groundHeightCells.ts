import type { Pixels } from "./groundCompose";

/**
 * Mark bilinear cells whose entire quarter-texel derivative footprint is constant.
 * Height is then constant for all four derivative samples, regardless of channel
 * heights or border thresholds. Conservative 4x4 support includes clamped edges.
 */
export function constantHeightCells(
  mask: Pixels<ArrayBufferLike>,
  borderWidths: readonly number[],
  edge: number,
): Uint8Array {
  const { width, height, data } = mask;
  const horizontal = new Uint8Array(width * height);
  const result = new Uint8Array(horizontal.length);
  // Bilinear interpolation stays inside the four source values. If all
  // channels remain on the same saturated side of both smoothstep ramps,
  // all four derivative heights are identical even across a soft mask gradient.
  const low = borderWidths.map((w) =>
    Math.min(Math.max(0.5 - w - edge, 0.05), Math.max(0.5 + w - edge, 0.05)),
  );
  const high = borderWidths.map((w) => {
    const end = (center: number) => {
      const lo = Math.max(center - edge, 0.05),
        hi = Math.min(center + edge, 0.95);
      return lo + Math.max(hi - lo, 1e-6);
    };
    return Math.max(end(0.5 - w), end(0.5 + w));
  });
  const classes = new Int16Array(width * height);
  for (let i = 0; i < classes.length; i++) {
    let code = 0;
    for (let c = 0; c < 4; c++) {
      const value = data[i * 4 + c] / 255;
      if (value < low[c] - 1e-12) continue;
      if (value > high[c] + 1e-12) code |= 1 << c;
      else {
        code = -1;
        break;
      }
    }
    classes[i] = code;
  }
  const same = (a: number, b: number) =>
    classes[a] >= 0 && classes[a] === classes[b];
  for (let y = 0; y < height; y++) {
    const row = y * width;
    for (let x = 0; x < width; x++) {
      const at = row + x;
      horizontal[at] = Number(
        same(at, row + Math.max(0, x - 1)) &&
          same(at, row + Math.min(width - 1, x + 1)) &&
          same(at, row + Math.min(width - 1, x + 2)),
      );
    }
  }
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const at = y * width + x;
      if (!horizontal[at]) continue;
      let uniform = true;
      for (let dy = -1; dy <= 2; dy++) {
        const other = Math.max(0, Math.min(height - 1, y + dy)) * width + x;
        if (!horizontal[other] || !same(at, other)) {
          uniform = false;
          break;
        }
      }
      result[at] = Number(uniform);
    }
  }
  return result;
}
