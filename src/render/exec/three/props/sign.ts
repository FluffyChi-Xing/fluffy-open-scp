import { buildModelProp, tagSemantic, type PropHandler } from "./build";

/** 招牌/待售牌等的静态牌面（状态层另由 building state 决定是否出现）。 */
export const executeSignProp: PropHandler = async (plan, payload, resources) =>
  tagSemantic(await buildModelProp(plan, payload, resources), "sign");
