import { buildModelProp, tagSemantic, type PropHandler } from "./build";

/** 垃圾/垃圾桶：槽位数量由 GarbageDisplayCount 等资源 bin 驱动（渲染全画）。 */
export const executeWasteProp: PropHandler = async (plan, payload, resources) =>
  tagSemantic(await buildModelProp(plan, payload, resources), "waste");
