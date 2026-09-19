/**
 * UI 工作台菜单导出：把内存中的菜单条目编辑装配成游戏可读的覆盖资源。
 *
 * Mod 语义 = 对原包的 diff 级覆盖（同 TGI 覆盖、新 TGI 新增）。一个菜单条目
 * 覆盖三类资源：
 *   1. 菜单条目 property   0x00B1B104 / 0x09878A01 / <条目 instance>
 *   2. 槽位图标 PNG        0x2F7D0004 / 0x40E02400 / <图标 instance>（128×128）
 *   3. rollover 大图 JPG   0x3F8662EA / 0x00000000 / <条目 instance>（454×263）
 * 文案（名称/描述/解锁提示/统计标签）以 diff 形式写入 locale 表覆盖：
 *   0x0A98EAF0 / 0x02FABF01 / <表 id>（0x6C969DEE 名称、0x50AA0BEA 描述、
 *   0x4B54417A 解锁提示），资源体 = UTF-8 BOM + 紧凑 JSON。
 *
 * 造价/预算（kPropSCUnitConstructionCost / kPropSCUnitMaintenanceCost）在
 * GlassBox 模拟规则侧，属性覆盖不可达，不在本导出范围内（工作台内存值
 * 仍保留，走 overlay JSON 通道）。
 */
import { encodePropertyFile, type PropEntry } from "./prop-format";
import { writeUncompressedOverlay, type DbpfEntry } from "./dbpf-writer";

export const MENU_PROP_TYPE = 0x00b1b104;
export const MENU_GROUP = 0x09878a01;
export const ICON_TYPE = 0x2f7d0004;
export const ICON_GROUP = 0x40e02400;
export const MARQUEE_TYPE = 0x3f8662ea;
export const LOCALE_TYPE = 0x0a98eaf0;
export const LOCALE_GROUP = 0x02fabf01;
/** 一级 plop 菜单容器（既有 8A01 条目的 parent）。 */
export const PLOP_MENU_PARENT = 0xaf042e9a;

/** 菜单条目属性哈希（与 ui_menu_survey / prop_dump 对拍的定案字段）。 */
export const HASH = {
  parent: 0x00b2cccb,
  title: 0x0a09f5fa,
  desc: 0x0a09f5fb,
  iconNormal: 0x09756950,
  hardGate: 0x0975695f,
  toolIconKey: 0x0977aa8f,
  uiCategory: 0x0db9fc63,
  uiPosition: 0x0dc1e3e0,
  marquee: 0x0ddeee56,
  unlockString: 0x0de84ddc,
  unlockTargetAmount: 0x0de84dd3,
  unitEffectTitle: 0x0eb1fc05,
  unitEffectParameter: 0x0eb1fc22,
} as const;

/** 文案表 id（游戏原表，diff 覆盖）。 */
export const LOCALE_TABLES = {
  toolName: 0x6c969dee,
  toolDesc: 0x50aa0bea,
  toolUnlock: 0x4b54417a,
} as const;

/** 导出用的一条菜单条目（工作台内存视图 → 这里）。 */
export interface MenuExportEntry {
  /** 菜单条目 instance（u32）。新条目由工作台生成。 */
  instance: number;
  /** 归属的一级分类 instance 列表（0x0DB9FC63）。 */
  uiCategories: number[];
  uiPosition: number;
  /** locale 引用（新字符串 id 由工作台分配）。 */
  titleStringId?: number;
  descStringId?: number;
  unlockStringId?: number;
  /** 槽位图标 instance（ICON_TYPE/ICON_GROUP 资源）；缺省不写图标键。 */
  iconInstance?: number;
  /** locked → 写 hardGate 键（值沿用观察到的形态）。 */
  locked?: boolean;
  /** 原始图片字节（PNG / JPEG），缺省跳过对应资源。 */
  iconPng?: Uint8Array;
  marqueeJpg?: Uint8Array;
}

export interface LocaleString {
  table: number;
  id: number;
  text: string;
}

/** 单条菜单条目 → 菜单条目 property 资源字节。 */
export function buildMenuPropertyResource(entry: MenuExportEntry): Uint8Array {
  const values: PropEntry[] = [
    {
      hash: HASH.parent,
      valueType: "key",
      values: [{ kind: "key", instance: PLOP_MENU_PARENT, typeId: MENU_PROP_TYPE, group: 0x48e1eb00 }],
    },
    {
      hash: HASH.title,
      valueType: "text",
      values: [{ kind: "text", tableId: LOCALE_TABLES.toolName, instanceId: entry.titleStringId ?? 0 }],
    },
    {
      hash: HASH.uiCategory,
      valueType: "key",
      array: true,
      values: entry.uiCategories.map((instance) => ({ kind: "key" as const, instance, typeId: 0, group: 0 })),
    },
    {
      hash: HASH.uiPosition,
      valueType: "int32",
      values: [{ kind: "int32", value: entry.uiPosition }],
    },
    {
      hash: HASH.marquee,
      valueType: "key",
      values: [{ kind: "key", instance: entry.instance, typeId: MARQUEE_TYPE, group: 0 }],
    },
  ];
  if (entry.descStringId != null) {
    values.push({
      hash: HASH.desc,
      valueType: "text",
      values: [{ kind: "text", tableId: LOCALE_TABLES.toolDesc, instanceId: entry.descStringId }],
    });
  }
  if (entry.iconInstance != null) {
    const iconKey = { kind: "key" as const, instance: entry.iconInstance, typeId: ICON_TYPE, group: ICON_GROUP };
    values.push({ hash: HASH.toolIconKey, valueType: "key", values: [iconKey] });
    values.push({ hash: HASH.iconNormal, valueType: "key", values: [iconKey] });
  }
  if (entry.locked) {
    values.push({ hash: HASH.hardGate, valueType: "key", values: [{ kind: "key", instance: entry.instance, typeId: 0, group: 0 }] });
  }
  if (entry.unlockStringId != null) {
    values.push({
      hash: HASH.unlockString,
      valueType: "text",
      values: [{ kind: "text", tableId: LOCALE_TABLES.toolUnlock, instanceId: entry.unlockStringId }],
    });
    values.push({ hash: HASH.unlockTargetAmount, valueType: "int32", values: [{ kind: "int32", value: 1 }] });
  }
  return encodePropertyFile(values);
}

/** locale 表覆盖资源（BOM + 紧凑 JSON，只含本次改动的字符串）。 */
export function buildLocaleResource(strings: LocaleString[], tableId: number): Uint8Array {
  const table = strings.filter((s) => s.table === tableId);
  const ordered = [...table].sort((a, b) => a.id - b.id);
  const parts = ordered.map((s) => `${JSON.stringify(sprintf32(s.id))}:${JSON.stringify(s.text)}`);
  const json = `{${parts.join(",")}}`;
  // UTF-8 BOM 前缀（SimCity locale 资源固定 3 字节）
  return Uint8Array.from([0xef, 0xbb, 0xbf, ...new TextEncoder().encode(json)]);
}

export interface MenuExportInput {
  entries: MenuExportEntry[];
  locale: LocaleString[];
}

export interface MenuExportResult {
  /** 覆盖包（.package，DBPF 容器，未压缩资源）。 */
  package: Uint8Array;
  /** 仅菜单条目 property 资源（单条时适用）；多条见 package。 */
  property: Uint8Array;
}

export function buildMenuExport(input: MenuExportInput): MenuExportResult {
  const entries: DbpfEntry[] = [];
  for (const entry of input.entries) {
    entries.push({
      type: MENU_PROP_TYPE,
      group: MENU_GROUP,
      instance: entry.instance,
      data: buildMenuPropertyResource(entry),
    });
    if (entry.iconPng && entry.iconInstance != null) {
      entries.push({ type: ICON_TYPE, group: ICON_GROUP, instance: entry.iconInstance, data: entry.iconPng });
    }
    if (entry.marqueeJpg) {
      entries.push({ type: MARQUEE_TYPE, group: 0, instance: entry.instance, data: entry.marqueeJpg });
    }
  }
  for (const tableId of new Set(input.locale.map((s) => s.table))) {
    entries.push({
      type: LOCALE_TYPE,
      group: LOCALE_GROUP,
      instance: tableId,
      data: buildLocaleResource(input.locale, tableId),
    });
  }
  return {
    package: writeUncompressedOverlay(entries),
    property: buildMenuPropertyResource(input.entries[0]),
  };
}

/** dataURL（data:image/png;base64,…）→ 字节。 */
export function dataUrlToBytes(dataUrl: string): Uint8Array {
  const base64 = dataUrl.slice(dataUrl.indexOf(",") + 1);
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export function sprintf32(value: number): string {
  return `0x${(value >>> 0).toString(16).padStart(8, "0")}`;
}

/** 浏览器下载辅助（Tauri webview 同样可用）。 */
export function downloadBytes(bytes: Uint8Array, filename: string, mime = "application/octet-stream"): void {
  const copy = new Uint8Array(bytes); // 拷贝以脱离 ArrayBuffer 池（部分内核限制）
  const blob = new Blob([copy], { type: mime });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 4000);
}
