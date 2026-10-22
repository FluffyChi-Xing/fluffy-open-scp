/** 程序化建筑 demo 的文档契约（openscp.procedural-building/1）。
 *
 * 生成器只负责模型组装（盒子套件 + 材质槽），已有游戏资产的挂接走
 * Property Editor 的 unit 编辑层（openscp.lot-asset/1），导出时以
 * `lotAsset` 字段整体嵌入——两份 schema 各自演进，互不感知对方细节。
 */
export const PROCEDURAL_BUILDING_SCHEMA =
  "openscp.procedural-building/1" as const;

export const PROCEDURAL_GENERATOR_ID = "facade-kit-v2" as const;
export const PROCEDURAL_GENERATOR_VERSION = 2 as const;

export type Vec3 = [number, number, number];

export interface TgiRef {
  type: number;
  group: number;
  instance: number;
}

/** 生成器内部的部件分组与缺省色。UI 使用一个 Property TGI 选择整套
 * building4 材质；保留旧 raster 绑定类型以兼容已有文档和底层调用。 */
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

/** A coarse building mass in metres. Rotation is around the Z axis. */
export interface BuildingBlock {
  id: string;
  center: [number, number, number];
  size: [number, number, number];
  rotation: number;
  /** A fixed plan junction; rotating a connected wing moves its centre around it. */
  junction?: [number, number];
}

/** A rectangular cavity in a block's local coordinates. */
export interface BuildingVoid {
  id: string;
  blockId: string;
  center: [number, number, number];
  size: [number, number, number];
}

export interface BlockMassParameters {
  blocks: BuildingBlock[];
  voids: BuildingVoid[];
}

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
  /** Horizontal gap between repeated facade window motifs, in metres. */
  windowSpacing?: number;
  /** Optional metric massing mode. Omitted documents keep the legacy bay kit. */
  blockMass?: BlockMassParameters;
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
  windowSpacing: 0,
};

export const DEFAULT_BLOCK_MASS: BlockMassParameters = {
  blocks: [
    { id: "block-1", center: [0, 0, 60], size: [72, 54, 120], rotation: 0 },
  ],
  voids: [],
};

/** 部件类别（决定几何归属与测试断言；渲染只消费盒体数据）。
 * 窗户/大门不建模——游戏里由立面贴图表达；阳台/老虎窗等出挑体量
 * 保留几何。 */
export type PartKind =
  | "mass"
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
  /** Rotation around Z in radians; absent means axis-aligned. */
  rotation?: number;
  /** Local cavities, applied before union with the other masses. */
  cutouts?: { center: Vec3; size: Vec3 }[];
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

export interface PropertyMaterialBinding {
  packageId: number;
  packagePath?: string;
  propertyTgi: TgiRef;
  modelPackageId: number;
  modelPackagePath?: string;
  modelTgi: TgiRef;
  lod: number;
}

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
  propertyMaterial?: PropertyMaterialBinding | null;
  lotAsset: Record<string, unknown> | null;
}

export function createProceduralBuildingDocument(
  parameters: ProceduralBuildingParameters,
  seed: number,
  textures: SlotBindingMap = {},
  lotAsset: Record<string, unknown> | null = null,
  propertyMaterial: PropertyMaterialBinding | null = null,
): ProceduralBuildingDocument {
  return {
    $schema: PROCEDURAL_BUILDING_SCHEMA,
    generator: {
      id: PROCEDURAL_GENERATOR_ID,
      version: PROCEDURAL_GENERATOR_VERSION,
      seed,
      parameters: JSON.parse(
        JSON.stringify(parameters),
      ) as ProceduralBuildingParameters,
    },
    textures: { ...textures },
    lotAsset,
    propertyMaterial,
  };
}

/** 建筑足迹（米；与生成器的水平布局一致）。 */
export function buildingFootprint(parameters: ProceduralBuildingParameters): {
  width: number;
  depth: number;
} {
  if (parameters.blockMass?.blocks.length) {
    const points: [number, number][] = [];
    for (const block of parameters.blockMass.blocks) {
      const [w, d] = block.size;
      const c = Math.cos((block.rotation * Math.PI) / 180);
      const s = Math.sin((block.rotation * Math.PI) / 180);
      for (const x of [-w / 2, w / 2])
        for (const y of [-d / 2, d / 2])
          points.push([
            block.center[0] + x * c - y * s,
            block.center[1] + x * s + y * c,
          ]);
    }
    return {
      width:
        Math.max(...points.map(([x]) => x)) -
        Math.min(...points.map(([x]) => x)),
      depth:
        Math.max(...points.map(([, y]) => y)) -
        Math.min(...points.map(([, y]) => y)),
    };
  }
  return { width: parameters.baysX * 3, depth: parameters.baysY * 3 };
}
