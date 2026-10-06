import type { LotModelLodRef, LotUnitDto } from "@/api/tauri";
import { unitId } from "./unitGizmos";

/**
 * 前端活体 schema 文档（openscp.lot-asset/1）：编辑态的唯一 JSON 投影面。
 *
 * 与后端引擎 pe_schema.rs build_schema 同构——差异仅在取值来源：后端从
 * 「session 原始数据 + 编辑层覆盖」合成，本文件直接从 shell 的
 * effectiveUnits（已合并 overrides/fieldOverrides/增删）投影，因此文档
 * 里的值即渲染链路正在消费的生效值，编辑任何东西 JSON 实时可读。
 *
 * 字段白名单逐字对齐 unit_json 的 kind 分支；units[].id 与 unitId()
 * （= Rust unit_id）同一命名空间，未来构建管线可直接消费/回写。
 */

/**
 * kind 特有字段白名单（= pe_schema.rs unit_json 的 kind 分支）。
 *
 * 【扩展点，用户指令 2026-10-06：schema 引擎须留足余地适应不同元素节点
 * 的定制元数据】未来按组件族放开编辑（灯源亮度/类型/颜色、effect 参数、
 * spawner 外观……）时的三处联动：
 *   1. 后端 pe_schema.rs unit_json 的 kind 白名单（权威结构）；
 *   2. 本表（前端投影同步）；
 *   3. PropertyEditorProperties 的「组件元数据」节（divider 分节按族
 *      追加编辑项，scaleEditable → per-kind editable 判定）。
 * 白名单外的自由字段落在 fields 透传位（hash → value），schema 无需改版。
 */
const KIND_KEYS: Record<string, string[]> = {
  light: [
    "lightType",
    "color",
    "outerRadius",
    "innerRadius",
    "diffuse",
    "length",
    "cullDistance",
    "isVolumetric",
    "debugName",
  ],
  effect: ["effectId", "enabled"],
  decal: ["category", "scale", "depth", "materialData"],
  prop: ["bin", "slot", "resourceId", "scale", "modelTgi", "modelPackageId"],
  pathPoint: ["point", "tangent", "pointIndex"],
  spawner: ["id", "count", "countRandom", "agent"],
};

const hex32 = (value: number) =>
  `0x${(value >>> 0).toString(16).padStart(8, "0").toUpperCase()}`;

/** DTO → schema 单元对象（fields 透传位含原始属性行）。 */
export function schemaUnitJson(unit: LotUnitDto): Record<string, unknown> {
  const object: Record<string, unknown> = {
    id: unitId(unit),
    kind: unit.kind,
    visible: true,
  };
  if ("transform" in unit && unit.transform) {
    object.transform = { matrix: unit.transform.matrix, flags: 15 };
  }
  for (const key of KIND_KEYS[unit.kind] ?? []) {
    const value = (unit as unknown as Record<string, unknown>)[key];
    if (value !== undefined) object[key] = value;
  }
  const fields = unit.fields ?? [];
  if (fields.length) {
    const map: Record<string, unknown> = {};
    for (const field of fields) {
      map[
        `0x${(field.hash >>> 0).toString(16).padStart(8, "0").toUpperCase()}`
      ] = field.value ?? "";
    }
    object.fields = map;
  }
  return object;
}

/** 文档输入：shell 的会话数据 + 生效单元集 + 视图状态。 */
export interface SchemaDocInput {
  assetName: string | null;
  tgi: { typeId: number; group: number; instance: number };
  modelLods: (LotModelLodRef | null)[];
  lotSize: [number, number] | null;
  lotTilePeriod: [number, number] | null;
  lotPlacement: number[] | null;
  lotBaseTile: number;
  lotColors: [number, number, number, number][];
  lotColorsAuthored: boolean[];
  lotBorderColors: [number, number, number][];
  lotBorderWidths: number[];
  lotBorderPatternIndices: number[];
  lotOverlayBoxOffset: [number, number] | null;
  lotModelBBoxCenter: [number, number] | null;
  units: LotUnitDto[];
  hiddenUnitIds: string[];
  groups: Record<string, boolean>;
}

/** 组装活体文档（与 build_schema 文档序一致；纯函数，computed 消费）。 */
export function buildSchemaDoc(input: SchemaDocInput): Record<string, unknown> {
  const hidden = new Set(input.hiddenUnitIds);
  return {
    $schema: "openscp.lot-asset/1",
    version: 1,
    meta: {
      name: input.assetName,
      source: {
        tgi: {
          typeId: hex32(input.tgi.typeId),
          group: hex32(input.tgi.group),
          instance: hex32(input.tgi.instance),
        },
      },
    },
    lot: {
      size: input.lotSize,
      tilePeriod: input.lotTilePeriod,
      placement: input.lotPlacement,
      baseTile: input.lotBaseTile,
      colors: input.lotColors.map((rgba, index) => ({
        rgba,
        authored: input.lotColorsAuthored[index] ?? false,
      })),
      borderColors: input.lotBorderColors,
      borderWidths: input.lotBorderWidths,
      borderPatterns: input.lotBorderPatternIndices,
      overlayBoxOffset: input.lotOverlayBoxOffset,
      modelBBoxCenter: input.lotModelBBoxCenter,
      mask: { kind: "source" },
    },
    model: {
      lods: input.modelLods.map((lod, index) => ({
        lod: index + 1,
        ref: lod
          ? {
              kind: "tgi",
              tgi: {
                typeId: hex32(lod.tgi.typeId),
                group: hex32(lod.tgi.group),
                instance: hex32(lod.tgi.instance),
              },
            }
          : null,
      })),
    },
    units: input.units.map((unit) => {
      const json = schemaUnitJson(unit);
      if (hidden.has(unitId(unit))) json.visible = false;
      return json;
    }),
    editor: {
      groups: input.groups,
      unitVisibility: Object.fromEntries(
        input.hiddenUnitIds.map((id) => [id, false]),
      ),
    },
  };
}
