/**
 * 贴花 frame 执行器：把纯计划转成旧视口消费的 three 结构。
 *
 * 只做数据形态转换（数组 → Vector3/Matrix4），不做任何判定——判定全部在
 * `plan/decal.ts`。迁移期视口仍使用 origin、axisX/Y/Z、matrix、sizeX/Y 字段。
 */
import type * as ThreeNamespace from "three";
import type { DecalFramePlan } from "../../plan/decal";

export interface DecalFrameObject {
  readonly origin: ThreeNamespace.Vector3;
  readonly axisX: ThreeNamespace.Vector3;
  readonly axisY: ThreeNamespace.Vector3;
  readonly axisZ: ThreeNamespace.Vector3;
  readonly matrix: ThreeNamespace.Matrix4;
  readonly sizeX: number;
  readonly sizeY: number;
  readonly boxDepth: number;
}

export function decalFrameObject(
  THREE: typeof ThreeNamespace,
  plan: DecalFramePlan,
): DecalFrameObject {
  const origin = new THREE.Vector3(plan.origin[0], plan.origin[1], plan.origin[2]);
  const axisX = new THREE.Vector3(plan.axisX[0], plan.axisX[1], plan.axisX[2]);
  const axisY = new THREE.Vector3(plan.axisY[0], plan.axisY[1], plan.axisY[2]);
  const axisZ = new THREE.Vector3(plan.axisZ[0], plan.axisZ[1], plan.axisZ[2]);
  const matrix = new THREE.Matrix4()
    .makeBasis(axisX, axisY, axisZ)
    .setPosition(origin);
  return {
    origin,
    axisX,
    axisY,
    axisZ,
    matrix,
    sizeX: plan.sizeX,
    sizeY: plan.sizeY,
    boxDepth: plan.boxDepth,
  };
}
