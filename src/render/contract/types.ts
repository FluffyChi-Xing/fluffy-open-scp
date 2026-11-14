/**
 * 渲染管线公共契约（新基线，2026-10-31）。
 *
 * 约束：
 * - 本层不 import three / 不碰 DOM / 无模块级可变状态；
 * - 所有描述符必须 JSON-safe（payload/THREE 对象只能通过 key 引用）；
 * - 执行器在 `src/render/exec/three/`，是唯一接触 three 的层。
 */

export type Vec3 = readonly [number, number, number];
/** 与 three 同序：[x, y, z, w] */
export type Quat = readonly [number, number, number, number];

/** Prop 脚本/资源语义（bin 只是绘制组，不带类型）。 */
export type PropSemantic =
  | "tree"
  | "vehicle"
  | "waste"
  | "sign"
  | "product"
  | "static";

/** 单元变换（WPF 行主序 12 floats 分解结果；语义与 three decompose 一致）。 */
export interface TransformSpec {
  readonly position: Vec3;
  readonly quaternion: Quat;
  readonly scale: Vec3;
}

/**
 * 缩放语义（取代旧 `userData.incrementalScale` 的 number | "keepScale" 字符串哨兵）：
 * - matrix：对象 scale 就是矩阵分解结果；
 * - baked：DTO scale 已烘焙进对象 scale，增量更新时按 factor 补乘；
 * - intrinsic：对象尺寸自持（公告板/合成小人），只跟位置/朝向。
 */
export type ScaleSemantics =
  | { readonly kind: "matrix" }
  | { readonly kind: "baked"; readonly factor: number }
  | { readonly kind: "intrinsic" };

/** 资产引用：执行器用它在上下文里取 LOTM 载荷。 */
export type AssetRef =
  | { readonly kind: "resource"; readonly resourceId: number }
  | { readonly kind: "instance"; readonly key: string };

interface UnitPlanBase {
  readonly id: string;
  readonly unitIndex: number;
  /** DTO 独立 scale 字段（半宽语义；null = 未设置）。 */
  readonly dtoScale: number | null;
  readonly transform: TransformSpec | null;
  readonly scale: ScaleSemantics;
}

/** 真实模型的 prop（车辆/杂件）。 */
export interface PropUnitPlan extends UnitPlanBase {
  readonly kind: "prop";
  readonly asset: AssetRef;
  /** 脚本/资源语义（tree/vehicle/waste/sign/product/static）。 */
  readonly semantic: PropSemantic;
}

/** 树：resolved = treeIds 命中；direct = 拖入的直挂树模型。 */
export interface TreeUnitPlan extends UnitPlanBase {
  readonly kind: "tree";
  readonly source: "resolved" | "direct";
  readonly asset: AssetRef;
  readonly seed: number;
}

/** spawner 人群预览。 */
export interface SpawnerUnitPlan extends UnitPlanBase {
  readonly kind: "spawner";
}

export type UnitPlan = PropUnitPlan | TreeUnitPlan | SpawnerUnitPlan;

/** 计划层输入：只读、与 three 无关。 */
export interface UnitPlanContext {
  readonly renderMode: "default" | "refined";
  readonly treeIds: ReadonlySet<number>;
  readonly propModels: ReadonlyMap<number, unknown>;
  readonly addedModelPayloads?: ReadonlyMap<string, unknown>;
  readonly treeModelPayloadCount: number;
  readonly simPartsCount: number;
}
