import type { TerrainField } from "./map-graph";
import type { Region3DData } from "./region-map";
export function terrainToRegion(field: TerrainField): Region3DData {
  const png = (height: boolean) => {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = field.size;
    const ctx = canvas.getContext("2d")!,
      pixels = ctx.createImageData(field.size, field.size);
    for (let i = 0; i < field.heights.length; i++) {
      const raw = Math.round(
        Math.max(0, Math.min(1, field.heights[i]!)) * 65535,
      );
      // Shared terrain material computes grass from sqrt(soil * moisture).
      // These are authored preview ecology channels, not measured game reserves.
      pixels.data.set(
        height
          ? [raw >>> 8, raw & 255, 0, 255]
          : [field.grass[i]!, field.trees[i]!, field.grass[i]!, 255],
        i * 4,
      );
    }
    ctx.putImageData(pixels, 0, 0);
    return canvas.toDataURL("image/png").split(",")[1]!;
  };
  return {
    size: field.size,
    metersPerPixel: field.extent / field.size,
    originWorld: [0, 0],
    waterZ: field.water,
    heightDiv: 65535 / field.elevation,
    heightBias: 0,
    desert: false,
    displayName: null,
    displayNameEn: null,
    plots: [],
    brushes: [],
    heightPngBase64: png(true),
    groundPngBase64: png(false),
  };
}
