import type { Region3DData } from "@/lib/region-map";

export const STATE_RESOURCE_IDS: Record<string, number> = {
  coal: 0xd779c976,
  ore: 0xd74af22b,
  oil: 0x76d10ff2,
  waterTable: 0x0f92dc56,
  forest: 0x1d52be3e,
  soil: 0x28f896a4,
  wind: 0x5b9db0f3,
  radiation: 0x1d716f2e,
  groundPollution: 0xf69df1c6,
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
  return lower === "watertable" || lower === "citywatertable"
    ? "waterTable"
    : kind;
}

/** Resample only inside observed city squares. Alpha distinguishes no data from zero. */
export function stateResourceTiles(data: Region3DData, kind: string, size: number) {
  const id = STATE_RESOURCE_IDS[canonicalResourceKind(kind)];
  const [originX, originY] = data.originWorld;
  const spacing = data.metersPerPixel;
  return (data.staticLayers?.sources ?? []).flatMap((source) => {
    const map = source.maps.find((m) => m.id === id);
    if (!map || !source.bounds) return [];
    const [x0, y0, x1, y1] = source.bounds;
    const left = Math.max(0, Math.ceil((x0 - originX) / spacing));
    const right = Math.min(size, Math.ceil((x1 - originX) / spacing));
    const top = Math.max(0, Math.ceil((y0 - originY) / spacing));
    const bottom = Math.min(size, Math.ceil((y1 - originY) / spacing));
    return left < right && top < bottom
      ? [{ site: source.site, bounds: source.bounds, map, left, right, top, bottom }]
      : [];
  });
}

export type ResourceTile = ReturnType<typeof stateResourceTiles>[number];

export function paintStateResourceTile(
  pixels: Uint8Array, size: number, data: Region3DData, tile: ResourceTile,
  changedRow?: (start: number, count: number) => void,
) {
  const [originX, originY] = data.originWorld;
  const [x0, y0, x1, y1] = tile.bounds;
  const { map, left, right, top, bottom } = tile;
  const spacing = data.metersPerPixel;
  const index = (position: number, min: number, max: number) =>
    Math.max(0, Math.min(map.size - 1, Math.floor((position - min) / (max - min) * map.size)));
  const columns = Array.from({ length: right - left }, (_, i) => index(originX + (left + i) * spacing, x0, x1));
  for (let y = top; y < bottom; y++) {
    const row = index(originY + y * spacing, y0, y1) * map.size;
    let first = right;
    let last = left;
    for (let x = left; x < right; x++) {
      const value = map.values[row + columns[x - left]!] ?? 0;
      const i = (y * size + x) * 4;
      const red = Math.round(Math.max(0, Math.min(65535, value)) / 65535 * 255);
      if (pixels[i] === red && pixels[i + 3] === 255) continue;
      pixels[i] = red;
      pixels[i + 3] = 255;
      first = Math.min(first, x);
      last = x;
    }
    if (first <= last) changedRow?.((y * size + first) * 4, (last - first + 1) * 4);
  }
}

export function stateResourcePixels(data: Region3DData, kind: string, size: number): Uint8Array {
  const pixels = new Uint8Array(size * size * 4);
  for (const tile of stateResourceTiles(data, kind, size)) paintStateResourceTile(pixels, size, data, tile);
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
      kind: kind === "waterTable" ? "cityWaterTable" : kind,
      coverage: map?.values.length
        ? (100 * map.values.filter((v) => v > 0).length) / map.values.length
        : null,
    };
  });
}
