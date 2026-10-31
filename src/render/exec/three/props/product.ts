import { buildModelProp, tagSemantic, type PropHandler } from "./build";

/** 工厂产品/仓库货物：槽位数量由产品资源 bin 驱动（渲染全画）。 */
export const executeProductProp: PropHandler = async (plan, payload, resources) =>
  tagSemantic(await buildModelProp(plan, payload, resources), "product");
