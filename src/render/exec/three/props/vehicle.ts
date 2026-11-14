import { buildModelProp, tagSemantic, type PropHandler } from "./build";

/** 车辆：脚本资源走 Vehicle Models/自 key；材质仍由 prop 链按车漆处理。 */
export const executeVehicleProp: PropHandler = async (plan, payload, resources) =>
  tagSemantic(await buildModelProp(plan, payload, resources), "vehicle");
