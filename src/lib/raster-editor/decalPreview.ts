import type { EngineFamily } from "@/lib/decalEngineMaterials";
import type { RasterProfile } from "./schema";
import type { RasterDocument } from "./document";

export function decalPreviewFamily(profile: RasterProfile): EngineFamily {
  if (profile === "decal-sdf" || profile === "decal-sign") return "sdf";
  if (profile === "decal-graffiti") return "sign";
  if (profile === "decal-interior") return "hole";
  return "clip";
}

/** DataTexture UV origin is bottom-left; editor rows start at the top. */
export function decalPreviewPixels(doc: RasterDocument): Uint8Array {
  const result = new Uint8Array(doc.pixels.length);
  const stride = doc.width * 4;
  for (let row = 0; row < doc.height; row++)
    result.set(
      doc.pixels.subarray(row * stride, (row + 1) * stride),
      (doc.height - row - 1) * stride,
    );
  return result;
}
