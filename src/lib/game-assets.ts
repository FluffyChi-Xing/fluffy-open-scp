/**
 * 游戏资产查询：把布局树里的路径引用解析成玩家安装包里的资源。
 *
 * 解析规则（游戏 ResourceMan 路径寻址，字节等价对拍定案）由后端
 * `game_asset` 命令实现：instance = FNV-1(小写去扩展名)、group = 0，
 * 在玩家全部 package 的合并索引里取 PNG/JPG 字节。
 * 非 Tauri 环境（浏览器预览）返回 null，调用方回退仓库内置资源。
 */
import { command, TauriUnavailableError } from "@/api/tauri";

const cache = new Map<string, string>();

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
    if (error instanceof TauriUnavailableError) return null;
    return null;
  }
}
