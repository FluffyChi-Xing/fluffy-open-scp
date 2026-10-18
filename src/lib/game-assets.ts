/**
 * 游戏资产查询：把布局树里的路径引用解析成玩家安装包里的资源。
 *
 * 解析规则（游戏 ResourceMan 路径寻址，字节等价对拍定案）由后端
 * `game_asset` 命令实现：instance = FNV-1(小写去扩展名)、group = 0，
 * 在玩家全部 package 的合并索引里取 PNG/JPG 字节。
 * 非 Tauri 环境（浏览器预览）返回 null，调用方回退仓库内置资源。
 */
import { command } from "@/api/tauri";
import { isTauri } from "@/api";

const cache = new Map<string, string>();

/** 菜单条目的图标/大图走完整 TGI 引用（toolIconKey/marquee 键的目标资源）。 */
export async function gameAssetTgiDataUrl(
  type: number,
  instance: number,
  group?: number,
): Promise<string | null> {
  const key = `tgi:${type.toString(16)}:${instance.toString(16)}:${group ?? "*"}`;
  const cached = cache.get(key);
  if (cached) return cached;
  try {
    const result = await command<{ dataBase64: string; mime: string }>("game_asset", {
      request: { tgi: { type, instance, group } },
    });
    const url = `data:${result.mime};base64,${result.dataBase64}`;
    cache.set(key, url);
    return url;
  } catch (error) {
    return null;
  }
}

export async function gameAssetDataUrl(refPath: string): Promise<string | null> {
  const cached = cache.get(refPath);
  if (cached) return cached;
  try {
    const result = await command<{ dataBase64: string; mime: string }>("game_asset", {
      request: { path: refPath },
    });
    const url = `data:${result.mime};base64,${result.dataBase64}`;
    cache.set(refPath, url);
    return url;
  } catch (error) {
    return null;
  }
}

export interface PropertyCopyRequest {
  sourceInstance: number;
  newInstance: number;
  uiCategories: number[];
  titleStringId?: number;
  descStringId?: number;
  unlockStringId?: number;
  position?: number;
  iconTgi?: { type: number; group: number; instance: number };
  marqueeTgi?: { type: number; group: number; instance: number };
}

/** 菜单条目全量 property 深拷贝（家族键随源条目保留），返回编码后的
 * property 资源字节（base64）。后端读玩家安装包里的源条目并做受控变换。 */
export async function gamePropertyCopy(
  request: PropertyCopyRequest,
): Promise<{ dataBase64: string; keyCount: number } | null> {
  if (!isTauri()) return null;
  return await command<{ dataBase64: string; keyCount: number }>("game_property_copy", {
    request,
  });
}
