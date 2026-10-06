import type * as ThreeNamespace from "three";
import type { LotModelPayload, LotUnitDto } from "@/api/tauri";
import type { SunEnvRefs } from "../refinedRender";
import type { SimPart } from "../simAssets";

/**
 * 单元渲染器分发层（2026-10-06 拆分）。
 *
 * 背景：精细模式的树/prop/spawner 装配逻辑原本内联在 Viewport 的巨型
 * if-else 链里，任何一族调整都可能波及他族（车漆/树色/小人多轮互相回归）。
 * 现按组件拆为独立渲染器：每个渲染器自持「适用判定 + 装配 + 降级链」，
 * 经 `renderRefinedAssetUnit` 分发；返回 null = 不适用/资产缺席，调用方
 * 统一退标记锥（buildUnitObject）。
 *
 * 约定：
 * - 新组件渲染器放本目录，注册进 index.ts 的 RENDERERS，不改 Viewport；
 * - 渲染器内部不得修改 ctx / 共享可变状态；
 * - 灯光与贴花因深度耦合视口内部状态（光源计数、建筑代理、遥测统计），
 *   暂留 Viewport 原位，未入分发。
 */

/** 精细模式资产渲染上下文：渲染器所需的全部外部输入（只读）。 */
export interface UnitRenderContext {
  THREE: typeof ThreeNamespace;
  renderMode: "default" | "refined";
  /** 树：模型树载荷（descriptor 配置表 impostor 源）与图集公告板。 */
  treeModelPayloads: LotModelPayload[];
  treeAtlasPng: string | null;
  treeIds: Set<number>;
  /** prop：resourceID → LOTM 载荷。 */
  propModels: Map<number, LotModelPayload>;
  /** 日夜环境（车漆/tint 着色链）。 */
  envRefs: SunEnvRefs | null;
  /** spawner：小人部件资产（全局，进程级缓存）。 */
  simParts: SimPart[];
}

export type UnitRenderer = (
  ctx: UnitRenderContext,
  unit: LotUnitDto,
) => Promise<ThreeNamespace.Object3D | null>;
