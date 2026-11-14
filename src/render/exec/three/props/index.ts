import type * as ThreeNamespace from "three";
import type { LotModelPayload } from "@/api/tauri";
import type { PropUnitPlan } from "../../../contract/types";
import type { ModelPropResources } from "./build";
import { executeProductProp } from "./product";
import { executeSignProp } from "./sign";
import { executeStaticProp } from "./static";
import { executeVehicleProp } from "./vehicle";
import { executeWasteProp } from "./waste";

/**
 * Prop 语义分流：每个处理器只负责一种语义；全部仍会渲染（不做状态显隐）。
 * 返回 null 表示载荷/模型缺失，调用方统一退标记锥。
 */
export async function executePropBySemantic(
  plan: PropUnitPlan,
  payload: LotModelPayload,
  resources: ModelPropResources,
): Promise<ThreeNamespace.Object3D | null> {
  switch (plan.semantic) {
    case "vehicle":
      return executeVehicleProp(plan, payload, resources);
    case "waste":
      return executeWasteProp(plan, payload, resources);
    case "sign":
      return executeSignProp(plan, payload, resources);
    case "product":
      return executeProductProp(plan, payload, resources);
    case "tree":
    case "static":
    default:
      return executeStaticProp(plan, payload, resources);
  }
}
