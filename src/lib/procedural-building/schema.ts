/** 程序化建筑 demo 的文档契约（openscp.procedural-building/1）。
 *
 * 生成器只负责模型组装（盒子套件 + 材质槽），已有游戏资产的挂接走
 * Property Editor 的 unit 编辑层（openscp.lot-asset/1），导出时以
 * `lotAsset` 字段整体嵌入——两份 schema 各自演进，互不感知对方细节。
 */
export const PROCEDURAL_BUILDING_SCHEMA = "openscp.procedural-building/1" as const;

export const PROCEDURAL_GENERATOR_ID = "facade-kit-v2" as const;
export const PROCEDURAL_GENERATOR_VERSION = 1 as const;

export type Vec3 = [number, number, number];

export interface TgiRef {
  type: number;
  group: number;
  instance: number;
}

/** 材质槽：生成的每个部件归属一个槽，槽可绑定游戏 raster 贴图。
 * 窗户不建模（游戏里窗户是立面贴图 + tint 链表达的），wall 槽绑
 * 带窗立面贴图即可。 */
export type MaterialSlotId = "wall" | "trim" | "roof" | "ground";

export interface MaterialSlot {
  id: MaterialSlotId;
  /** 未绑定贴图时的 sRGB 缺省色（烘焙为顶点色）。 */
  defaultColor: string;
}

export const MATERIAL_SLOTS: readonly MaterialSlot[] = [
  { id: "wall", defaultColor: "#cdbfa8" },
  { id: "trim", defaultColor: "#948a79" },
  { id: "roof", defaultColor: "#6d6960" },
  { id: "ground", defaultColor: "#9b9486" },
];

export type RoofStyle = "flat" | "mansard";
export type BalconyMode = "none" | "featured" | "all";
export type DetailStyle = "off" | "uniform" | "alternating" | "random";

export interface ProceduralBuildingParameters {
  /** 面宽开间数（1 开间 = 3 m）。 */
  baysX: number;
  /** 进深开间数。 */
  baysY: number;
  /** 地上标准层数（底层额外）。 */
  floors: number;
  /** 底层层高（m）。 */
  groundFloorHeight: number;
  /** 标准层高（m）。 */
  floorHeight: number;
  roofStyle: RoofStyle;
  balconies: BalconyMode;
  detail: DetailStyle;
}

export const DEFAULT_PARAMETERS: ProceduralBuildingParameters = {
  baysX: 6,
  baysY: 4,
  floors: 5,
  groundFloorHeight: 4.2,
  floorHeight: 3.2,
  roofStyle: "mansard",
  balconies: "featured",
  detail: "alternating",
};

/** 部件类别（决定几何归属与测试断言；渲染只消费盒体数据）。
 * 窗户/大门不建模——游戏里由立面贴图表达；阳台/老虎窗等出挑体量
 * 保留几何。 */
export type PartKind =
  | "foundation"
  | "slab"
  | "wall"
  | "pier"
  | "balconySlab"
  | "balconyRail"
  | "cornice"
  | "roofSlab"
  | "mansardStep"
  | "dormer"
  | "bulkhead"
  | "pilaster"
  | "refend";

export interface BuildingPart {
  id: string;
  kind: PartKind;
  slot: MaterialSlotId;
  /** 盒体中心（米；Z-up，原点 = 地块中心/地面）。 */
  center: Vec3;
  /** 盒体尺寸（米，全轴对齐，无旋转）。 */
  size: Vec3;
}

/** 槽 → 游戏 raster 贴图绑定（选中时写入，导出随文档持久化）。 */
export interface SlotBinding {
  slot: MaterialSlotId;
  packageId: number;
  packagePath?: string;
  tgi: TgiRef;
  /** 显示名（raster 解析名或 instance hex）。 */
  label?: string;
}

export type SlotBindingMap = Partial<Record<MaterialSlotId, SlotBinding>>;

/** 导出文档：生成器参数 + 贴图绑定 + Property Editor 的 lot 资产文档。 */
export interface ProceduralBuildingDocument {
  $schema: typeof PROCEDURAL_BUILDING_SCHEMA;
  generator: {
    id: typeof PROCEDURAL_GENERATOR_ID;
    version: typeof PROCEDURAL_GENERATOR_VERSION;
    seed: number;
    parameters: ProceduralBuildingParameters;
  };
  textures: SlotBindingMap;
  lotAsset: Record<string, unknown> | null;
}

export function createProceduralBuildingDocument(
  parameters: ProceduralBuildingParameters,
  seed: number,
  textures: SlotBindingMap = {},
  lotAsset: Record<string, unknown> | null = null,
): ProceduralBuildingDocument {
  return {
    $schema: PROCEDURAL_BUILDING_SCHEMA,
    generator: {
      id: PROCEDURAL_GENERATOR_ID,
      version: PROCEDURAL_GENERATOR_VERSION,
      seed,
      parameters: { ...parameters },
    },
    textures: { ...textures },
    lotAsset,
  };
}

/** 建筑足迹（米；与生成器的水平布局一致）。 */
export function buildingFootprint(parameters: ProceduralBuildingParameters): {
  width: number;
  depth: number;
} {
  return { width: parameters.baysX * 3, depth: parameters.baysY * 3 };
}
