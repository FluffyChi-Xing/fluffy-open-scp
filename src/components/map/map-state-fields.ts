import type { Region3DData } from "@/lib/region-map";

export const STATE_RESOURCE_IDS: Record<string, number> = {
  coal: 0xd779c976,
  ore: 0xd74af22b,
  oil: 0x76d10ff2,
  waterTable: 0x0f92dc56,
  forest: 0x1d52be3e,
  soil: 0x28f896a4,
};
export function hasStateResource(
  data: Region3DData | null,
  kind: string,
): boolean {
  const id = STATE_RESOURCE_IDS[kind];
  return (
    id !== undefined &&
    !!data?.saveLayers?.sources.some(
      (s) => s.bounds && s.maps.some((m) => m.id === id),
    )
  );
}

/** SoilLayer is the saved absolute height field (also exported as 03E421F0).
 * Keep the original region outside city simulation bounds; saved roads use these grades.
 */
export function applySavedCityFields(
  data: Region3DData,
  raw: Uint16Array,
  eco: ImageData,
): void {
  const size = eco.width,
    spacing = data.metersPerPixel;
  for (const source of data.saveLayers?.sources ?? []) {
    if (!source.bounds) continue;
    const [x0, y0, x1, y1] = source.bounds;
    const height = source.maps.find((m) => m.id === 0x6f9f6ab3);
    const channels = [0x28f896a4, 0x1d52be3e, 0x0f92dc56].map((id) =>
      source.maps.find((m) => m.id === id),
    );
    for (
      let y = Math.max(0, Math.ceil((y0 - data.originWorld[1]) / spacing));
      y < Math.min(size, Math.ceil((y1 - data.originWorld[1]) / spacing));
      y++
    ) {
      for (
        let x = Math.max(0, Math.ceil((x0 - data.originWorld[0]) / spacing));
        x < Math.min(size, Math.ceil((x1 - data.originWorld[0]) / spacing));
        x++
      ) {
        const mx = (data.originWorld[0] + x * spacing - x0) / 16,
          my = (data.originWorld[1] + y * spacing - y0) / 16;
        const sample = (m: NonNullable<typeof height>) => {
          const ix = Math.min(m.size - 1, Math.floor(mx)),
            iy = Math.min(m.size - 1, Math.floor(my));
          const nx = Math.min(m.size - 1, ix + 1),
            ny = Math.min(m.size - 1, iy + 1),
            tx = mx - ix,
            ty = my - iy;
          return (
            (m.values[iy * m.size + ix] * (1 - tx) +
              m.values[iy * m.size + nx] * tx) *
              (1 - ty) +
            (m.values[ny * m.size + ix] * (1 - tx) +
              m.values[ny * m.size + nx] * tx) *
              ty
          );
        };
        if (height)
          raw[y * size + x] = Math.max(
            0,
            Math.min(65535, Math.round(sample(height))),
          );
        channels.forEach((m, c) => {
          if (m)
            eco.data[(y * size + x) * 4 + c] = Math.round(
              (Math.max(0, Math.min(65535, sample(m))) / 65535) * 255,
            );
        });
      }
    }
  }
}
/** Resample only inside observed city squares. Alpha distinguishes no data from zero. */
export function stateResourcePixels(
  data: Region3DData,
  kind: string,
  size: number,
): Uint8Array {
  const pixels = new Uint8Array(size * size * 4);
  const spacing = data.metersPerPixel;
  for (const source of data.saveLayers?.sources ?? []) {
    const map = source.maps.find((m) => m.id === STATE_RESOURCE_IDS[kind]);
    if (!map || !source.bounds) continue;
    const [x0, y0, x1, y1] = source.bounds;
    for (
      let y = Math.max(0, Math.ceil((y0 - data.originWorld[1]) / spacing));
      y < Math.min(size, Math.ceil((y1 - data.originWorld[1]) / spacing));
      y++
    ) {
      for (
        let x = Math.max(0, Math.ceil((x0 - data.originWorld[0]) / spacing));
        x < Math.min(size, Math.ceil((x1 - data.originWorld[0]) / spacing));
        x++
      ) {
        const mx = Math.floor(
          ((data.originWorld[0] + x * spacing - x0) / (x1 - x0)) * map.size,
        );
        const my = Math.floor(
          ((data.originWorld[1] + y * spacing - y0) / (y1 - y0)) * map.size,
        );
        const value = map.values[my * map.size + mx];
        const i = (y * size + x) * 4;
        pixels[i] = Math.round(
          (Math.max(0, Math.min(65535, value)) / 65535) * 255,
        );
        pixels[i + 3] = 255;
      }
    }
  }
  return pixels;
}
