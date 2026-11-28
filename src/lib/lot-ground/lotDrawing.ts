import { base64ToRgba, rgbaToBase64 } from "@/lib/raster-editor/encoders";
import type { RasterSchema } from "@/lib/raster-editor/schema";
import { maskSizeForLot, resampleNearest } from "./ground";

/** Round the imported width outward so the authored frontage is not shortened. */
export function importedLotSize([width, depth]: readonly [number, number]): [number, number] {
  return [Math.max(1, Math.ceil(width)), Math.max(1, Math.round(depth))];
}

/** Native masks may use a square texture for a rectangular world-space lot. */
export function prepareLotDrawing(
  schema: RasterSchema,
  size: [number, number] | null | undefined,
): RasterSchema {
  const image = schema.image;
  if (!image || !size) return schema;
  const metersPerPixel = Math.max(schema.sampling.metersPerPixel, Math.max(...size) / 4096);
  const { width, height } = maskSizeForLot(size, metersPerPixel);
  const resized = width !== image.width || height !== image.height;
  return {
    ...schema,
    image: resized ? {
      ...image, width, height,
      data: rgbaToBase64(resampleNearest(base64ToRgba(image.data), image.width, image.height, width, height)),
    } : image,
    sampling: { ...schema.sampling, metersPerPixel },
    lot: { ...schema.lot, size },
  };
}
