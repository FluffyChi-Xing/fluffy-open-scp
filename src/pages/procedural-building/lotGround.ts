import { onScopeDispose, shallowRef } from "vue";
import { tauriApi } from "@/api";
import type { LotEditorSession, Tgi } from "@/api/tauri";
import {
  base64ToRgba,
  rgbaBase64ToPngDataUrl,
  rgbaToBase64,
  fitSizeWithin,
} from "@/lib/raster-editor/encoders";
import { composeGroundPixels } from "@/pages/packages/components/property-editor/groundCompose";

export interface LotSelection {
  packageId: number;
  tgi: Tgi;
  label: string;
  lotPackageId: number;
  lotTgi: Tgi;
  lotSize: [number, number] | null;
}
export function lotKey(item: Pick<LotSelection, "lotPackageId" | "lotTgi">) {
  const t = item.lotTgi;
  return `${item.lotPackageId}:${t.typeId}:${t.group}:${t.instance}`;
}
/** Same flat layer composition as RasterPreviewPanel; never round-trip raw
 * channel weights through canvas (alpha would destroy RGB weights). */
export function lotLayerPixels(
  source: { rgbaBase64: string | null; width: number; height: number },
  colors: LotEditorSession["lotColors"],
  maxSize = 256,
) {
  if (!source.rgbaBase64 || source.width <= 0 || source.height <= 0)
    throw new Error("Missing Lot mask pixels");
  const mask = {
    data: base64ToRgba(source.rgbaBase64),
    width: source.width,
    height: source.height,
  };
  if (mask.data.length !== source.width * source.height * 4)
    throw new Error("Invalid Lot mask dimensions");
  return composeGroundPixels({
    mask,
    rawMask: mask,
    baseTile: null,
    lotColors: colors,
    lotBorderColors: null,
    lotBorderPatternIndices: null,
    lotBorderWidths: null,
    normalAtlas: null,
    tilesX: 1,
    tilesY: 1,
    outSize: fitSizeWithin(source.width * 4, source.height * 4, maxSize),
  });
}
export function lotLayerPng(
  source: Parameters<typeof lotLayerPixels>[0],
  colors: LotEditorSession["lotColors"],
  maxSize = 256,
) {
  const output = lotLayerPixels(source, colors, maxSize);
  return rgbaBase64ToPngDataUrl(
    rgbaToBase64(output.albedo),
    output.width,
    output.height,
  );
}
export async function loadLotGround(selection: LotSelection) {
  const [authored, raw] = await Promise.all([
    tauriApi.packages.readLotEditorSession(
      selection.lotPackageId,
      selection.lotTgi,
    ),
    tauriApi.raster.readSource(selection.packageId, selection.tgi),
  ]);
  if (!authored.lotMaskPng) throw new Error("Missing decoded Lot mask");
  // The session backend flips mask rows into world XY orientation. The raster
  // editor intentionally shows file rows, so only the picker uses raw directly.
  const flat = authored.lotMaskRawRgba
    ? lotLayerPng(
        { ...raw, rgbaBase64: authored.lotMaskRawRgba },
        authored.lotColors,
        1024,
      )
    : `data:image/png;base64,${authored.lotAlbedoPng ?? authored.lotMaskPng}`;
  const session: LotEditorSession = {
    ...authored,
    lotSize: authored.lotSize ?? selection.lotSize,
    // Keep the authored full-resolution default albedo. The generated flat
    // image is only the picker preview; the viewport uses backend albedo.
    units: [],
    decalTextures: [],
    pathPairs: [],
  };
  return { session, dataUrl: flat, selection };
}
/** Latest selection wins, including clear and component disposal. */
export function useLotGround(loader = loadLotGround) {
  const selected = shallowRef<Awaited<ReturnType<typeof loadLotGround>> | null>(
    null,
  );
  const loading = shallowRef(false);
  let generation = 0;
  function clear() {
    generation++;
    loading.value = false;
    selected.value = null;
  }
  async function select(selection: LotSelection) {
    const token = ++generation;
    loading.value = true;
    try {
      const result = await loader(selection);
      if (token !== generation) return false;
      selected.value = result;
      return true;
    } catch (error) {
      if (token !== generation) return false;
      throw error;
    } finally {
      if (token === generation) loading.value = false;
    }
  }
  onScopeDispose(clear);
  return { selected, loading, select, clear };
}
