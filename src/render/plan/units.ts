/**
 * 单元渲染计划（纯函数）：把 DTO 语义 + 资产可用性变成可执行描述符。
 *
 * 分发语义与旧 `renderers/{tree,prop,spawner}Renderer` **逐条对齐**：
 * 1. 仅精细模式生效（默认模式仍走标记锥兜底）；
 * 2. prop：treeIds 命中 → 树（resolved）；否则先查直挂载荷，无载荷即不适用；
 *    直挂模型且资产类别为 trees → 树（direct）；其余 → prop 真模型；
 * 3. spawner：资产（simParts）缺席时不适用。
 *
 * 返回 null = 不适用/资产缺席，调用方统一退 `buildUnitObject` 标记锥。
 */
import type { LotUnitDto } from "@/api/tauri";
import { assetCategory } from "@/pages/packages/components/property-editor/assetCategories";
import type {
  AssetRef,
  ScaleSemantics,
  TransformSpec,
  UnitPlan,
  UnitPlanContext,
} from "../contract/types";
import { rowMajor12ToTrs } from "../resolve/transform";
import { classifyProp } from "./propSemantic";

/** Unit 稳定 id（与旧 `unitGizmos.unitId` 同规则）。 */
export function unitPlanId(unit: LotUnitDto): string {
  if (unit.kind === "prop") return `prop:${unit.bin}:${unit.index}`;
  if (unit.kind === "decal") return `decal:${unit.category}:${unit.index}`;
  return `${unit.kind}:${unit.index}`;
}

/** 直挂模型 key（与旧 `assetCategories.directModelKey` 同规则）。 */
export function directAssetKey(unit: {
  modelTgi?: { typeId: number; group: number; instance: number } | null;
  modelPackageId?: number | null;
}): string {
  const tgi = unit.modelTgi;
  return tgi ? `${unit.modelPackageId}:${tgi.typeId}:${tgi.group}:${tgi.instance}` : "";
}

function scaleOf(unit: LotUnitDto): number | null {
  const value = (unit as { scale?: number | null }).scale;
  return typeof value === "number" ? value : null;
}

function transformOf(unit: LotUnitDto): TransformSpec | null {
  const transform = (unit as { transform?: { matrix: number[] } | null }).transform;
  return transform ? rowMajor12ToTrs(transform.matrix) : null;
}

function bakedSemantics(unit: LotUnitDto): ScaleSemantics {
  return { kind: "baked", factor: scaleOf(unit) ?? 1 };
}

/**
 * 单元 → 渲染计划（纯函数）。`ctx` 只含只读事实，不含 three 对象。
 */
export function planUnit(unit: LotUnitDto, ctx: UnitPlanContext): UnitPlan | null {
  if (ctx.renderMode !== "refined") return null;
  const id = unitPlanId(unit);
  const unitIndex = unit.index;
  const transform = transformOf(unit);

  if (unit.kind === "spawner") {
    if (ctx.simPartsCount === 0) return null;
    return {
      kind: "spawner",
      id,
      unitIndex,
      dtoScale: scaleOf(unit),
      transform,
      scale: { kind: "intrinsic" },
    };
  }
  if (unit.kind !== "prop") return null;

  const resourceId = typeof unit.resourceId === "number" ? unit.resourceId : null;

  // 1) 资源解析出的树（tree/tree_model）——优先于 prop 模型
  if (resourceId !== null && ctx.treeIds.has(resourceId)) {
    const asset: AssetRef = { kind: "resource", resourceId };
    return {
      kind: "tree",
      id,
      unitIndex,
      dtoScale: scaleOf(unit),
      transform,
      // 有 3D 形状载荷 = 模型树（baked）；否则回落公告板（intrinsic）
      scale:
        ctx.treeModelPayloadCount > 0
          ? bakedSemantics(unit)
          : { kind: "intrinsic" },
      source: "resolved",
      asset,
      seed: resourceId * 2654435761 + unit.index,
    };
  }

  // 2) 载荷查找：有 modelTgi 时只认直挂模型（旧语义：不回退 propModels）
  const directKey = unit.modelTgi ? directAssetKey(unit) : "";
  const hasDirectPayload = Boolean(
    directKey && ctx.addedModelPayloads?.has(directKey),
  );
  if (unit.modelTgi && !hasDirectPayload) return null;
  const hasResourcePayload =
    resourceId !== null && ctx.propModels.has(resourceId);
  if (!hasDirectPayload && !hasResourcePayload) return null;

  // 3) 直挂树：资产类别判定（旧 propRenderer 的兜底分支）
  if (
    unit.modelTgi &&
    assetCategory(unit.modelTgi.instance, unit.displayName) === "trees"
  ) {
    return {
      kind: "tree",
      id,
      unitIndex,
      dtoScale: scaleOf(unit),
      transform,
      scale: bakedSemantics(unit),
      source: "direct",
      asset: { kind: "instance", key: directKey },
      seed: unit.index,
    };
  }

  const asset: AssetRef = hasDirectPayload
    ? { kind: "instance", key: directKey }
    : { kind: "resource", resourceId: resourceId as number };
  return {
    kind: "prop",
    id,
    unitIndex,
    dtoScale: scaleOf(unit),
    transform,
    scale: bakedSemantics(unit),
    asset,
    semantic: classifyProp({
      resourceId,
      name: unit.displayName,
      treeIds: ctx.treeIds,
    }),
  };
}
