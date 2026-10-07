/**
 * 地图面板共享类型（前端视图与组件间）。
 */

export interface RegionSummary {
  group: string;
  displayName: string | null;
  displayNameEn: string | null;
  numericId: string;
  plotCount: number;
}

export interface ResourceLayer {
  /** 资源 kind（coal/ore/oil/...）。 */
  kind: string;
  /** RGBA PNG（裁剪框 1/2 尺寸，前端拉伸到裁剪框显示）。 */
  pngBase64: string;
}

export interface RegionRender {
  pngBase64: string;
  width: number;
  height: number;
  originWorld?: [number, number];
  metersPerPixel: number;
  waterPlane: number;
  desert: boolean;
  displayName: string | null;
  displayNameEn: string | null;
  plotCount: number;
  /** 城市地块中心（**PNG 像素坐标**，后端已换算）。 */
  plots: [number, number][];
  /** 资源画刷：目标 map 名 + 各 stamp（**PNG 像素坐标**）。 */
  brushes: [string, [number, number][]][];
  /** 资源分布图层（游戏数据视图同款等值线色带）。 */
  resourceLayers: ResourceLayer[];
}

/** 从画刷目标名提取资源 kind（"coalEcoMapBrushes" → "coal"）。 */
export function brushResourceKind(mapName: string): string {
  return mapName.replace(/EcoMapBrushes$/i, "");
}

/** 资源 kind → 覆盖层颜色（与游戏内资源视图色带对应）。 */
export const RESOURCE_COLORS: Record<string, string> = {
  coal: "#4a423c",
  oil: "#26221f",
  ore: "#f0b429",
  oreMetal: "#c9a227",
  watertable: "#2f86eb",
  soil: "#a06a3b",
  forest: "#2e9e44",
  desirability: "#e05656",
  desirabilitytwo: "#9b59b6",
  radiation: "#35c94f",
  groundpollution: "#a03cc8",
};

export function resourceColor(kind: string): string {
  return RESOURCE_COLORS[kind.toLowerCase()] ?? "#ff00ff";
}

/** 3D 预览地块（城市或伟工位）。 */
export interface Region3DPlot {
  x: number;
  y: number;
  /** 场地基准面高（米）。 */
  z: number;
  uid: string;
  name: string | null;
  nameEn?: string | null;
  kind: "city" | "greatwork";
}

/** map_panel_region_3d 返回的区域 3D 数据。 */
export interface Region3DData {
  /** 高度 PNG（RGBA：R=raw 高 8 位、G=低 8 位，无损 16-bit 编码）。 */
  heightPngBase64: string;
  /** 生态数据 PNG（RGBA：R=土壤、G=森林、B=地下水、A=255；非 sRGB）。 */
  groundPngBase64: string;
  /** Game terrain diffuse faces, loaded from the adjacent App package. */
  terrainTextures?: Partial<Record<"dirt" | "grass" | "cliff" | "sand", string>>;
  /** 场边长（2048）。 */
  size: number;
  metersPerPixel: number;
  originWorld: [number, number];
  /** 水面世界高（米，-870）。 */
  waterZ: number;
  /** z = raw / heightDiv + heightBias。 */
  heightDiv: number;
  heightBias: number;
  desert: boolean;
  displayName: string | null;
  displayNameEn: string | null;
  plots: Region3DPlot[];
  /** 资源画刷清单（目标 map 名 + stamp 世界坐标）。 */
  brushes: [string, [number, number][]][];
}
