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
  const id = STATE_RESOURCE_IDS[canonicalResourceKind(kind)];
  return (
    id !== undefined &&
    !!data?.staticLayers?.sources.some(
      (s) => s.bounds && s.maps.some((m) => m.id === id),
    )
  );
}

/** Keep resource names stable across package keys and UI labels. */
export function canonicalResourceKind(kind: string): string {
  const lower = kind.toLowerCase();
  return lower === "watertable" ? "waterTable" : kind;
}

/** Resample only inside observed city squares. Alpha distinguishes no data from zero. */
export function stateResourcePixels(
  data: Region3DData,
  kind: string,
  size: number,
): Uint8Array {
  const pixels = new Uint8Array(size * size * 4);
  const spacing = data.metersPerPixel;
  for (const source of data.staticLayers?.sources ?? []) {
    const map = source.maps.find(
      (m) => m.id === STATE_RESOURCE_IDS[canonicalResourceKind(kind)],
    );
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
        const mx = Math.max(
          0,
          Math.min(
            map.size - 1,
            Math.floor(
              ((data.originWorld[0] + x * spacing - x0) / (x1 - x0)) * map.size,
            ),
          ),
        );
        const my = Math.max(
          0,
          Math.min(
            map.size - 1,
            Math.floor(
              ((data.originWorld[1] + y * spacing - y0) / (y1 - y0)) * map.size,
            ),
          ),
        );
        const value = map.values[my * map.size + mx] ?? 0;
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

/** Measured occupied cells, not brush count or a fabricated tonnage. */
export function cityResourceSummary(data: Region3DData | null, uid?: string) {
  const source = data?.staticLayers?.sources.find(
    (s) => s.site === uid && s.bounds,
  );
  return Object.entries(STATE_RESOURCE_IDS).map(([kind, id]) => {
    const map = source?.maps.find((m) => m.id === id);
    return {
      kind,
      coverage: map?.values.length
        ? (100 * map.values.filter((v) => v > 0).length) / map.values.length
        : null,
    };
  });
}
