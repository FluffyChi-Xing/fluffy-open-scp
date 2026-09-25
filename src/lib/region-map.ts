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

/** 画刷清单（后端 `map_panel_list_brushes`；stamps 为世界坐标米）。 */
export interface BrushList {
  /** 清单 property 的 instance（hex 串），编辑命令的定位键。 */
  instance: string;
  name: string;
  mapName: string | null;
  stamps: [number, number][];
}

/** 单条目 overlay 写回结果（`map_panel_set_water_level` / `map_panel_edit_brush_stamps`）。 */
export interface OverlayWriteResult {
  entryCount: number;
  sizeBytes: number;
  outPath: string;
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
