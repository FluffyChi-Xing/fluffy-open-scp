/** Convert a row between the north-up canvas/game raster and the south-first preview mask. */
export function oppositeMaskRow(row: number, height: number): number {
  return height - 1 - row;
}

/** Convert the row order without reordering columns or interpreting alpha as opacity. */
export function reverseMaskRows(
  pixels: Uint8ClampedArray,
  width: number,
  height: number,
): Uint8ClampedArray<ArrayBuffer> {
  const stride = width * 4;
  const result = new Uint8ClampedArray(pixels.length);
  for (let row = 0; row < height; row += 1) {
    result.set(
      pixels.subarray(row * stride, (row + 1) * stride),
      oppositeMaskRow(row, height) * stride,
    );
  }
  return result;
}
