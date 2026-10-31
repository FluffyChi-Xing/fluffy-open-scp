import { buildModelProp, tagSemantic, type PropHandler } from "./build";

/** 普通静态摆件：不透明漫反射，无 alpha cutout（车辆裁穿教训）。 */
export const executeStaticProp: PropHandler = async (plan, payload, resources) =>
  tagSemantic(await buildModelProp(plan, payload, resources), "static");
