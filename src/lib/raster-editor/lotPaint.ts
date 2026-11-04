import type { Rgba } from './tools';

/** Existing toolbar order, independent of engine channel priority. */
export const LOT_PAINT_CHANNELS = [3, 0, 1, 2] as const;

/** A > B > G > R. Zero weights expose the separate base material. */
export function lotRegion(pixel: Rgba, widths: readonly number[] = []): number {
  for (const channel of [3, 2, 1, 0]) {
    const width = widths[channel] ?? 0;
    if (pixel[channel] / 255 > 0.5 - width) {
      return channel * 2 + (pixel[channel] / 255 <= 0.5 + width ? 1 : 0);
    }
  }
  return -1;
}

export function lotPaintDisplay(
  pixels: Uint8ClampedArray,
  colors: readonly (readonly number[])[],
  widths: readonly number[] = [],
  borders: readonly (readonly number[])[] = [],
): Uint8ClampedArray {
  const output = new Uint8ClampedArray(pixels.length);
  for (let at = 0; at < pixels.length; at += 4) {
    const region = lotRegion(Array.from(pixels.subarray(at, at + 4)) as Rgba, widths);
    const channel = Math.floor(region / 2);
    const color = region < 0 ? [90, 90, 90]
      : region % 2 ? borders[channel] ?? [156, 156, 156]
      : colors[channel] ?? [128, 128, 128];
    output.set([color[0], color[1], color[2], 255], at);
  }
  return output;
}
