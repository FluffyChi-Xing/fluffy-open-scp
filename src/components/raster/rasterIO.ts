import { i18n } from "@/locales";
const t = i18n.global.t;
import { tauriApi } from "@/api";
import type { PropertyDocumentSummary, Tgi } from "@/api/tauri";
import { base64ToRgba } from "@/lib/raster-editor/encoders";
import {
  parseRasterSchema,
  type RasterSchema,
} from "@/lib/raster-editor/schema";
import { encodeRgbaPng } from "@/lib/raster-editor/png";

export function downloadJson(value: unknown, name: string) {
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], { type: "application/json" }),
  );
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
export function downloadRasterAsset(value: RasterSchema) {
  const schema = parseRasterSchema(value);
  if (schema.parameters) {
    downloadJson(schema.parameters, `${schema.name}.json`);
    return;
  }
  const image = schema.image!;
  const bytes = encodeRgbaPng(
    image.width,
    image.height,
    base64ToRgba(image.data),
  );
  const url = URL.createObjectURL(
    new Blob([new Uint8Array(bytes)], { type: "image/png" }),
  );
  const link = document.createElement("a");
  link.href = url;
  link.download = `${schema.name}.png`;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
function resource(typeId: number): Tgi {
  return {
    typeId,
    group: 0,
    instance: crypto.getRandomValues(new Uint32Array(1))[0] || 1,
  };
}
function instance(text: string): number | undefined {
  if (!text.trim()) return undefined;
  if (!/^(?:0x)?[\da-f]{1,8}$/i.test(text.trim()))
    throw new Error(t("rasterEditor.text036"));
  return parseInt(text.replace(/^0x/i, ""), 16);
}
function storageColor(hex: string): [number, number, number, number] {
  const convert = (v: number) =>
    (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4) / 2;
  return [
    convert(parseInt(hex.slice(1, 3), 16) / 255),
    convert(parseInt(hex.slice(3, 5), 16) / 255),
    convert(parseInt(hex.slice(5, 7), 16) / 255),
    0,
  ];
}
export async function saveRasterCopy(
  value: RasterSchema,
  kind: "raster" | "lot" | "decal",
): Promise<string | null> {
  const s = parseRasterSchema(value);
  if (!s.image || s.profile === "slot0")
    throw new Error(t("rasterEditor.text037"));
  const { width, height, data: rgbaBase64 } = s.image;
  if (kind === "lot" && s.profile !== "lot-mask")
    throw new Error(t("rasterEditor.text038"));
  if (kind === "lot" && s.sampling.metersPerPixel !== 0.75)
    throw new Error(t("rasterEditor.text039"));
  const atlas = s.decal.atlas;
  if (
    kind === "decal" &&
    (!s.profile.startsWith("decal-") || atlas?.packageId == null || !atlas.tgi)
  )
    throw new Error(t("rasterEditor.text040"));
  const entryId = kind === "decal" ? instance(s.decal.entryId) : undefined;
  const replaceInstance =
    kind === "decal" ? instance(s.decal.replaceInstance) : undefined;
  if (kind === "raster" && s.profile === "lot-mask" && s.lot.target) {
    if (s.source?.packageId == null || !s.source.tgi)
      throw new Error(t("rasterEditor.text041"));
    const original = await tauriApi.raster.readRgba(
      s.source.packageId,
      s.source.tgi,
    );
    if (original.width !== width || original.height !== height)
      throw new Error(t("rasterEditor.text042"));
  }
  const outputPath = await tauriApi.packages.saveFile(
    `${s.name.replace(/[<>:"/\\|?*]/g, "-")}.package`,
    "package",
  );
  if (!outputPath) return null;
  const common = {
    width,
    height,
    rgbaBase64,
    generateMips: s.sampling.generateMips,
    outputPath,
  };
  if (kind === "lot") {
    await tauriApi.raster.createLotOverlay({
      ...common,
      rasterTgi: resource(0x2f4e681c),
      lotTgi: resource(0x00b1b104),
      colors: s.lot.colors,
    });
  } else if (kind === "decal") {
    await tauriApi.raster.registerDecalEntry({
      ...common,
      atlas: { packageId: atlas!.packageId!, tgi: atlas!.tgi! },
      rasterTgi: resource(0x2f4e681c),
      entryId: {
        typeId: s.decal.entryTgi?.typeId ?? 0,
        group: s.decal.entryTgi?.group ?? 0,
        instance: replaceInstance ?? entryId ?? resource(0).instance,
      },
      aspectRatio: s.decal.aspectRatio,
      colors: s.decal.colors.map(storageColor),
      replaceInstance,
    });
  } else {
    const target = s.profile === "lot-mask" ? s.lot.target : null;
    await tauriApi.raster.saveOverlay({
      ...common,
      tgi: s.source?.tgi ?? resource(0x2f4e681c),
      companionProperty:
        target?.tgi && target.packageId != null
          ? { packageId: target.packageId, tgi: target.tgi }
          : undefined,
    });
  }
  return outputPath;
}
export async function loadLotCopy(
  summary: PropertyDocumentSummary & { packageId: number },
  packages: number[],
) {
  const mask =
    summary.lotMaskResolved ??
    (summary.lotMask ? { ...summary.lotMask, typeId: 0x2f4e681c } : null);
  if (!mask) throw new Error(t("rasterEditor.text043"));
  const candidates = [
    ...new Set(
      [summary.lotMaskPackageId, summary.packageId, ...packages].filter(
        (id): id is number => id != null,
      ),
    ),
  ];
  for (const id of candidates) {
    const image = await tauriApi.raster.readRgba(id, mask).catch(() => null);
    if (!image?.decodable || !image.rgbaBase64) continue;
    const material = await tauriApi.raster.readLotMaterial(
      summary.packageId,
      summary.tgi,
    );
    return {
      image: {
        width: image.width,
        height: image.height,
        pixels: base64ToRgba(image.rgbaBase64),
      },
      material,
      source: {
        name: `Lot 0x${summary.tgi.instance.toString(16)}`,
        packageId: id,
        tgi: mask,
      },
      target: {
        name: `Lot 0x${summary.tgi.instance.toString(16)}`,
        packageId: summary.packageId,
        tgi: { ...summary.tgi },
      },
    };
  }
  throw new Error(t("rasterEditor.text044"));
}
