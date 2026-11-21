/** View-only relief on dyed ground; the calibrated compose core keeps flat albedo. */
export function enhanceTintDetail(
  albedo: Uint8ClampedArray,
  normals: Uint8ClampedArray,
  width: number,
  height: number,
  mask: { width: number; height: number; data: Uint8ClampedArray },
): void {
  for (let y = 0; y < height; y += 1) {
    const maskY = Math.min(
      mask.height - 1,
      Math.floor(((y + 0.5) * mask.height) / height),
    );
    for (let x = 0; x < width; x += 1) {
      const maskX = Math.min(
        mask.width - 1,
        Math.floor(((x + 0.5) * mask.width) / width),
      );
      const maskAt = (maskY * mask.width + maskX) * 4;
      if (
        mask.data[maskAt] < 128 &&
        mask.data[maskAt + 1] < 128 &&
        mask.data[maskAt + 2] < 128 &&
        mask.data[maskAt + 3] < 128
      )
        continue;
      const at = (y * width + x) * 4;
      const nx = normals[at] / 127.5 - 1;
      const ny = normals[at + 1] / 127.5 - 1;
      const shade = Math.min(1.4, Math.max(0.6, 1 - 0.55 * (nx + ny)));
      for (let c = 0; c < 3; c += 1)
        albedo[at + c] = Math.round(albedo[at + c] * shade);
    }
  }
}
