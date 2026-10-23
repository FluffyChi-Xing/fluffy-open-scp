import type { LotMaterialTextures, LotModelPayload } from "@/api/tauri";
export interface BuildingTextureExportFile {
  filename: string;
  bytes: Uint8Array<ArrayBuffer>;
  mime: string;
}

/**
 * Export the six building material slots in the same order used by the LOTM
 * decoder. Slot 0 is the float parameter table (there is no PNG for it), so it
 * is emitted as JSON; slots 1-5 preserve the original PNG bytes.
 */
export function buildBuildingTextureSlotFiles(
  payload: LotModelPayload,
  baseName: string,
): BuildingTextureExportFile[] {
  const files: BuildingTextureExportFile[] = [];
  const manifest: Array<Record<string, unknown>> = [];
  payload.materials.forEach((material, materialIndex) => {
    const prefix = `${baseName}-mat${String(materialIndex + 1).padStart(2, "0")}`;
    const slot0 = {
      slot: 0,
      kind: "float4-parameter-table",
      paramCols: material.paramCols,
      paramsF32: material.paramsF32 ? Array.from(material.paramsF32) : null,
    };
    files.push({ filename: `${prefix}-slot0-params.json`, bytes: new TextEncoder().encode(JSON.stringify(slot0, null, 2)), mime: "application/json" });
    manifest.push({
      material: materialIndex,
      slot: 0,
      file: `${prefix}-slot0-params.json`,
      format: "json",
      available: Boolean(material.paramsF32?.length),
    });
    const slots: [number, keyof LotMaterialTextures][] = [
      // Building slot 1 is the raw tint/color-control map. baseColorPng is a
      // renderer-derived diffuse bake and is intentionally left out here.
      [1, "tintPng"],
      [2, "normalPng"],
      [3, "shaderPng"],
      [4, "palettePng"],
      [5, "interiorPng"],
    ];
    for (const [slot, key] of slots) {
      const bytes = material[key];
      if (!(bytes instanceof Uint8Array) || bytes.byteLength === 0) continue;
      files.push({ filename: `${prefix}-slot${slot}.png`, bytes, mime: "image/png" });
      manifest.push({
        material: materialIndex,
        slot,
        file: `${prefix}-slot${slot}.png`,
        format: "png",
        available: true,
      });
    }
  });
  files.push({ filename: `${baseName}-texture-slots.json`, bytes: new TextEncoder().encode(JSON.stringify({ version: 1, slots: manifest }, null, 2)), mime: "application/json" });
  return files;
}
