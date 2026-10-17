import { tauriApi } from "@/api";
import type { LotEditorSession } from "@/api/tauri";
import type { RasterSource } from "@/lib/raster-editor/schema";

/** Borrow only shared atlases; another Lot must never replace the edited layout. */
export async function loadLotPreviewSession(
  target: RasterSource | null,
  packageIds: number[],
  cancelled: () => boolean,
): Promise<{
  bound: LotEditorSession | null;
  atlas: Partial<LotEditorSession> | null;
}> {
  let bound: LotEditorSession | null = null;
  let lastError: unknown;
  const visited = new Set<string>();
  async function read(source: RasterSource) {
    if (!source.tgi || source.packageId == null || cancelled()) return null;
    const key = `${source.packageId}:${source.tgi.typeId}:${source.tgi.group}:${source.tgi.instance}`;
    if (visited.has(key)) return null;
    visited.add(key);
    try {
      return await tauriApi.packages.readLotEditorSession(
        source.packageId,
        source.tgi,
      );
    } catch (error) {
      lastError = error;
      return null;
    }
  }
  if (target?.tgi && target.packageId != null) {
    bound = await read(target);
    // A broken bound Lot should be reported, not replaced with another layout.
    if (!bound && !cancelled()) throw lastError;
    if (bound?.lotSurfacePng) return { bound, atlas: bound };
  }
  if (cancelled()) return { bound, atlas: null };
  try {
    const atlas = await tauriApi.raster.readLotPreviewAtlases();
    if (atlas?.lotSurfacePng) return { bound, atlas };
  } catch (error) {
    lastError = error;
  }
  for (const packageId of packageIds) {
    if (cancelled()) break;
    try {
      const documents = await tauriApi.raster.listPropertyDocuments(
        packageId,
        "lot",
      );
      for (const document of documents) {
        if (cancelled()) break;
        const candidate = await read({
          name: "",
          packageId,
          tgi: document.tgi,
        });
        if (candidate?.lotSurfacePng) return { bound, atlas: candidate };
      }
    } catch (error) {
      lastError = error;
    }
  }
  if (lastError && !cancelled()) throw lastError;
  return { bound, atlas: null };
}
