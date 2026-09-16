export type ResourceKind =
  "rw4" | "raster" | "property" | "text" | "media" | "other";

export const RW4_TYPE_ID = 0x2f4e681b;
export const RASTER_TYPE_ID = 0x2f4e681c;
export const PROPERTY_TYPE_ID = 0x00b1b104;
export const AUDIO_TYPE_ID = 0x0d9e5710;
export const WWISE_BANK_TYPE_ID = 0x0a4d8d09;
export const VIDEO_TYPE_ID = 0x376840d7;
export const TTF_TYPE_ID = 0x276ca4b9;
/**
 * Property 子类型判别位：`InstanceType = GroupContainer & 0xFFFF`。
 * 原 SCP `PackageReader/DataBaseIndex.cs`：
 *   `public uint InstanceType { get { return (_groupContainer & 0xffff); } } // mask 0000XXXX`
 * Group 的高 16 位是同一子类型的分卷编号（同一 lot 可见 0x40E1C000 / 0x42E1C000 多条），
 * **不参与判别**。取值表来自 `Views/valueConverters/InstanceTypeIconConverter.cs` 的
 * `PropertyFileTypeIds` 枚举。
 *
 * 注意：decal / prop / spawner **不是** property 子类型，而是 **Unit（0xC000）内部的
 * 单元种类**，靠特征列哈希区分（见 file-types.zh-CN.md「Unit 内部单元种类」）。
 */
/**
 * Property 资源的子类型 i18n key 表（回退用：demo 模式/后端未下发 semantic 时
 * 按 group 低 16 位直接查表；正常运行以后端 sc-properties::semantic 判定为
 * 单一真源，含结构判据与 Parent 继承，覆盖面更大）。
 * 原 11 项来自 `Views/valueConverters/InstanceTypeIconConverter.cs`；
 * 其余为普查实证家族（见 docs/overview/analyze/05-property-semantic-survey.md）。
 */
export const PROPERTY_INSTANCE_TYPES: Readonly<Record<number, string>> = {
  0xc000: "package.instanceType.unit",
  0xc600: "package.instanceType.agent",
  0xc400: "package.instanceType.network",
  0x8b7e: "package.instanceType.path",
  0xc900: "package.instanceType.menu2",
  0x8a01: "package.instanceType.menu",
  0xe000: "package.instanceType.mapLayer",
  0x2043: "package.instanceType.descriptor",
  0xb185: "package.instanceType.decalAtlas",
  0x1651: "package.instanceType.decalAtlas2",
  0x1652: "package.instanceType.decalAtlas3",
  0x2d00: "package.instanceType.agentVehicleModel",
  0xc100: "package.instanceType.resourceDef",
  0xe800: "package.instanceType.resourceEntry",
  0xc500: "package.instanceType.simAction",
  0x44f2: "package.instanceType.alert",
  0xc300: "package.instanceType.zoneColor",
  0xba03: "package.instanceType.menuCategory",
  0xeb00: "package.instanceType.toolBody",
  0xe900: "package.instanceType.utilityLine",
  0x0000: "package.instanceType.modelWrapper",
};

/** Decal Atlas（贴花图鉴）三册的 InstanceType。 */
export const DECAL_ATLAS_GROUP_TYPES: readonly number[] = [
  0xb185, 0x1651, 0x1652,
];

export function isDecalAtlasGroup(group: number): boolean {
  return DECAL_ATLAS_GROUP_TYPES.includes(group & 0xffff);
}

/**
 * Property 资源的子类型 i18n key；非 Property 或未知 InstanceType 返回 null。
 * 调用方需自行用 `t()` 翻译。
 */
export function propertyInstanceKey(
  typeId: number,
  group: number,
): string | null {
  if (typeId !== PROPERTY_TYPE_ID) return null;
  return PROPERTY_INSTANCE_TYPES[group & 0xffff] ?? null;
}
/** TGA / Cursor / Greyscale Map（8/32/16-bit）：需 Rust 解码为 PNG。 */
export const GENERIC_IMAGE_TYPE_IDS = [
  0x2f7d0006, // TGA
  0x02393756, // Cursor (ICO/CUR)
  0x03e421ec, // Greyscale 8-bit
  0x03e421ed, // Greyscale 32-bit
  0x03e421f0, // Greyscale 16-bit
] as const;

const extensionUrls = import.meta.glob("../assets/file-extensions/*.svg", {
  eager: true,
  import: "default",
  query: "?url",
}) as Record<string, string>;

const iconUrls = Object.fromEntries(
  Object.entries(extensionUrls).map(([path, url]) => [
    path.slice(path.lastIndexOf("/") + 1, -4).toLowerCase(),
    url,
  ]),
);

export const resourceKindMeta: Record<
  ResourceKind,
  { label: string; icon: string; fallback: string }
> = {
  rw4: { label: "package.rw4", icon: "rw4", fallback: "Box" },
  raster: { label: "package.raster", icon: "raster", fallback: "Image" },
  property: { label: "package.property", icon: "prop", fallback: "ListTree" },
  text: { label: "package.text", icon: "txt", fallback: "FileText" },
  media: { label: "package.media", icon: "mp4", fallback: "Film" },
  other: { label: "package.other", icon: "unknown", fallback: "FileQuestion" },
};

export function resourceKind(typeId: number): ResourceKind {
  if (typeId === RW4_TYPE_ID) return "rw4";
  if (typeId === RASTER_TYPE_ID) return "raster";
  if ((GENERIC_IMAGE_TYPE_IDS as readonly number[]).includes(typeId))
    return "raster";
  if (typeId === PROPERTY_TYPE_ID) return "property";
  if (typeId === AUDIO_TYPE_ID || typeId === WWISE_BANK_TYPE_ID || typeId === VIDEO_TYPE_ID)
    return "media";
  // 语言表 + 状态脚本（0x024A0E52，内容实证纯文本）走文本预览
  if (textPreviewLanguages[typeId] != null) return "text";
  return "other";
}

const textPreviewLanguages: Record<number, string> = {
  0x67771f5c: "javascript",
  0x2c978db6: "css",
  0xdd6233d6: "html",
  0x0469a3f7: "cpp",
  0x0a98eaf0: "json",
  0x024a0e52: "ini",
};

export function textPreviewLanguage(typeId: number): string | null {
  return textPreviewLanguages[typeId] ?? null;
}

const imageMimes: Record<number, string> = {
  0x2f7d0004: "image/png",
  0x3f8662ea: "image/jpeg",
  0x2f7d0007: "image/gif",
};

export function imageMimeForType(typeId: number): string | null {
  return imageMimes[typeId] ?? null;
}

export function resourceIconUrl(kind: ResourceKind): string | undefined {
  return iconUrls[resourceKindMeta[kind].icon];
}

export function extensionIconUrl(fileName: string): string | undefined {
  const extension = fileName.split(".").pop()?.toLowerCase() ?? "unknown";
  return iconUrls[extension] ?? iconUrls.unknown;
}
