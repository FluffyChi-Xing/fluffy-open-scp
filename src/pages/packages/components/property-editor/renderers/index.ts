import type * as ThreeNamespace from "three";
import type { LotUnitDto } from "@/api/tauri";
import { renderPropUnit } from "./propRenderer";
import { renderSpawnerUnit } from "./spawnerRenderer";
import { renderTreeUnit } from "./treeRenderer";
import type { UnitRenderContext, UnitRenderer } from "./types";

/**
 * 精细模式资产渲染分发：按注册序逐渲染器尝试，首个产出非 null 者胜出。
 * 渲染器自持适用判定（kind/模式/资产齐备），返回 null = 不适用或降级，
 * 调用方统一退标记锥（buildUnitObject）。
 *
 * 新组件渲染器：本目录建文件 + 注册进 RENDERERS 即可，不改 Viewport。
 */
const RENDERERS: UnitRenderer[] = [renderTreeUnit, renderPropUnit, renderSpawnerUnit];

export async function renderRefinedAssetUnit(
  ctx: UnitRenderContext,
  unit: LotUnitDto,
): Promise<ThreeNamespace.Object3D | null> {
  for (const render of RENDERERS) {
    const object = await render(ctx, unit);
    if (object) return object;
  }
  return null;
}

export type { UnitRenderContext, UnitRenderer };
