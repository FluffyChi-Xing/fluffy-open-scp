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
  /** 源分组过滤：同 instance 可能存在于多个包/分组，提供后精确命中。 */
  sourceGroup?: number;
  newInstance: number;
  /** 统计行模板重写（全量 Text 引用，写自有表）。 */
  statTitleRefs?: { table: number; instance: number }[];
  uiCategories: number[];
  titleStringId?: number;
  descStringId?: number;
  unlockStringId?: number;
  /** 源有解锁文案而副本被清空 → 删除解锁键；缺省保留源引用。 */
  removeUnlockString?: boolean;
  position?: number;
  iconTgi?: { type: number; group: number; instance: number };
  marqueeTgi?: { type: number; group: number; instance: number };
}

export async function gameMenuGroupsCompile(request: {
  parentInstance: number;
  parentGroup: number;
  newGroup: number;
  localeTable: number;
  defaultTitleStringId: number;
  children: { instance: number; titleStringId: number }[];
}): Promise<{ dataBase64: string; typeId: number; group: number; instance: number }[]> {
  return command("game_menu_groups_compile", { request });
}

/** 菜单条目全量 property 深拷贝（家族键随源条目保留），返回编码后的
 * property 资源字节（base64）。后端读玩家安装包里的源条目并做受控变换。 */
export async function gamePropertyCopy(
  request: PropertyCopyRequest,
): Promise<
  { dataBase64: string; keyCount: number; sourceGroup: number } | null
> {
  if (!isTauri()) return null;
  return await command<{
    dataBase64: string;
    keyCount: number;
    sourceGroup: number;
  }>("game_property_copy", {
    request,
  });
}

/** property 类型化读取（Sheet 深层编辑先读后改）。 */
export interface PropertyReadEntry {
  hash: number;
  array: boolean;
  values: {
    kind: string;
    int?: number;
    float?: number;
    text?: [number, number];
    key?: [number, number, number];
  }[];
}

export interface PropertyReadResult {
  typeId: number;
  group: number;
  instance: number;
  keys: PropertyReadEntry[];
}

export async function gamePropertyRead(request: {
  instance: number;
  group?: number;
}): Promise<PropertyReadResult | null> {
  if (!isTauri()) return null;
  return await command<PropertyReadResult>("game_property_read", { request });
}

export interface PropertyPatchFloat {
  hash: number;
  value: number;
}

export interface PropertyPatchTextRefs {
  hash: number;
  refs: { table: number; instance: number }[];
}

export interface PropertyPatchRequest {
  instance: number;
  group?: number;
  setFloats?: PropertyPatchFloat[];
  setInts?: { hash: number; value: number }[];
  setTextRefs?: PropertyPatchTextRefs[];
}

/** 类型化键补丁：读源 → 改值 → 规范编码，回显完整 TGI。 */
export async function gamePropertyPatch(
  request: PropertyPatchRequest,
): Promise<
  { dataBase64: string; typeId: number; group: number; instance: number } | null
> {
  if (!isTauri()) return null;
  return await command<{
    dataBase64: string;
    typeId: number;
    group: number;
    instance: number;
  }>("game_property_patch", { request });
}
