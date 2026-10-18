import { tauriApi } from "@/api";
import type { RasterPreviewData, Tgi } from "@/api/tauri";

/** 游戏 raster 贴图的目录与解码：程序化建筑的材质槽贴图来源。
 *
 * raster 条目 type = 0x2F4E681C；可解码性由后端裁定（pixFmt 21
 * A8R8G8B8 未压缩可出 PNG，DXT 压缩变体仅元数据）。
 */

export const RASTER_TYPE_ID = 0x2f4e681c;

export interface RasterCatalogEntry {
  packageId: number;
  tgi: Tgi;
  /** resolveNames 解析名；失败回退 instance hex。 */
  label: string;
  decodable: boolean;
  width: number | null;
  height: number | null;
}

export interface RasterPng {
  bytes: Uint8Array<ArrayBuffer>;
  dataUrl: string;
  preview: RasterPreviewData;
}

const pngCache = new Map<string, RasterPng | null>();

export function rasterKey(packageId: number, tgi: Tgi): string {
  return `${packageId}:${tgi.typeId}:${tgi.group}:${tgi.instance}`;
}

export function instanceLabel(tgi: Tgi): string {
  return `0x${tgi.instance.toString(16).toUpperCase().padStart(8, "0")}`;
}

/** 某个已打开包的 raster 目录（含显示名；名称解析失败不影响列表）。 */
export async function listRasterEntries(
  packageId: number,
  filter: string,
  limit = 400,
): Promise<RasterCatalogEntry[]> {
  const page = await tauriApi.packages.listResources(
    packageId,
    0,
    limit,
    filter.trim() ? filter.trim() : undefined,
    RASTER_TYPE_ID,
  );
  if (!page.items.length) return [];
  const names = new Map<string, string>();
  try {
    const resolved = await tauriApi.packages.resolveNames(
      packageId,
      page.items.map(item => item.tgi),
    );
    for (const entry of resolved) {
      if (!entry.displayName) continue;
      names.set(
        `${entry.tgi.typeId}:${entry.tgi.group}:${entry.tgi.instance}`,
        entry.displayName,
      );
    }
  } catch {
    // 名称解析失败回退 instance hex
  }
  return page.items.map(item => ({
    packageId,
    tgi: item.tgi,
    label:
      names.get(`${item.tgi.typeId}:${item.tgi.group}:${item.tgi.instance}`) ??
      instanceLabel(item.tgi),
    decodable: false,
    width: null,
    height: null,
  }));
}

/** 解码 raster 首 mip 为 PNG（composite 视图）；结果缓存；失败缓存 null。 */
export async function loadRasterPng(
  packageId: number,
  tgi: Tgi,
): Promise<RasterPng | null> {
  const key = rasterKey(packageId, tgi);
  if (pngCache.has(key)) return pngCache.get(key) ?? null;
  try {
    const preview = await tauriApi.packages.readRasterPreview(
      packageId,
      tgi,
      "composite",
    );
    if (!preview.decodable || !preview.pngBase64) {
      pngCache.set(key, null);
      return null;
    }
    const binary = atob(preview.pngBase64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
    const png: RasterPng = {
      bytes,
      dataUrl: `data:image/png;base64,${preview.pngBase64}`,
      preview,
    };
    pngCache.set(key, png);
    return png;
  } catch {
    pngCache.set(key, null);
    return null;
  }
}

/** 探测条目可解码性与尺寸（目录展示用；与 loadRasterPng 共享缓存）。 */
export async function probeRasterEntry(
  packageId: number,
  tgi: Tgi,
): Promise<{ decodable: boolean; width: number | null; height: number | null }> {
  const key = rasterKey(packageId, tgi);
  if (pngCache.has(key)) {
    const cached = pngCache.get(key);
    return cached
      ? { decodable: true, width: cached.preview.width, height: cached.preview.height }
      : { decodable: false, width: null, height: null };
  }
  try {
    const preview = await tauriApi.packages.readRasterPreview(packageId, tgi, "composite");
    if (!preview.decodable || !preview.pngBase64) {
      pngCache.set(key, null);
      return { decodable: false, width: null, height: null };
    }
    // 预览数据已有 base64——直接入缓存，避免 pick 时二次 IPC
    const binary = atob(preview.pngBase64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
    pngCache.set(key, {
      bytes,
      dataUrl: `data:image/png;base64,${preview.pngBase64}`,
      preview,
    });
    return { decodable: true, width: preview.width, height: preview.height };
  } catch {
    pngCache.set(key, null);
    return { decodable: false, width: null, height: null };
  }
}
