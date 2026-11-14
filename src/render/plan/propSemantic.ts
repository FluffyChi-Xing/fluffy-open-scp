/**
 * Prop 语义分类（纯函数）：只读资源事实，不碰 three/UI。
 *
 * 注意：bin 只是引擎的“资源绘制组”，不编码物体类型；真正的类型来自
 * prop 脚本资源（ResolvedPropModel.source）与已验证的 resourceId 映射。
 * 这里给 plan/exec 一个稳定的语义标签，用于分处理器渲染与属性面板展示；
 * 未知资源一律 other，不做贴图/名称的强制猜测。
 */
import type { AssetRef, PropSemantic } from "../contract/types";

export type { PropSemantic };

/** 已验证的 resourceId → 语义（来源 assetCategories.ts / 引擎普查）。 */
const KNOWN: Record<number, PropSemantic> = {
  0x0ab4dfe1: "vehicle",
  0x0ab4dfe2: "vehicle",
  0x0ab4dfe3: "vehicle",
  0x92bee95f: "vehicle",
  0x54ca89f0: "vehicle",
  0x85271637: "waste",
  0x903a704c: "waste",
  0x4df43690: "tree",
  0xc2fbd178: "tree",
  0x1113b131: "tree",
  0x89d658df: "tree",
};

export interface PropSemanticFacts {
  readonly resourceId: number | null;
  readonly name?: string;
  readonly treeIds: ReadonlySet<number>;
}

export function classifyProp(facts: PropSemanticFacts): PropSemantic {
  const { resourceId, name = "" } = facts;
  if (resourceId !== null && facts.treeIds.has(resourceId)) return "tree";
  if (resourceId !== null) {
    const known = KNOWN[resourceId >>> 0];
    if (known) return known;
  }
  if (/tree|plant|bush|palm|foliage|树|植被/i.test(name)) return "tree";
  if (/truck|vehicle|car(?:_|\b)|bus|train|车辆|汽车/i.test(name)) return "vehicle";
  if (/trash|dumpster|garbage|waste|垃圾/i.test(name)) return "waste";
  if (/sign|billboard|招牌|广告牌/i.test(name)) return "sign";
  if (/product|goods|cargo|货物|产品/i.test(name)) return "product";
  return "static";
}

/** 从已解析的资产引用与名称分类（exec/属性面板共用）。 */
export function classifyAssetRef(
  asset: AssetRef,
  name: string,
  treeIds: ReadonlySet<number>,
): PropSemantic {
  const resourceId = asset.kind === "resource" ? asset.resourceId : null;
  return classifyProp({ resourceId, name, treeIds });
}
