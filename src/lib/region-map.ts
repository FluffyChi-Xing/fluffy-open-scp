/**
 * 地图面板共享类型（前端视图与组件间）。
 */

export interface RegionSummary {
  group: string;
  displayName: string | null;
  numericId: string;
  plotCount: number;
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
  plotCount: number;
  brushes: [string, [number, number][]][];
}
