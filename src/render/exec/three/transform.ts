import type * as ThreeNamespace from "three";
import type { TransformSpec } from "../../contract/types";

/** 纯 TRS 应用（计划描述符 → three 对象变换）。 */
export function applyTransform(
  THREE: typeof ThreeNamespace,
  transform: TransformSpec | null,
  position: ThreeNamespace.Vector3,
  quaternion: ThreeNamespace.Quaternion,
  scale: ThreeNamespace.Vector3,
): void {
  void THREE;
  if (!transform) return;
  position.set(transform.position[0], transform.position[1], transform.position[2]);
  quaternion.set(
    transform.quaternion[0],
    transform.quaternion[1],
    transform.quaternion[2],
    transform.quaternion[3],
  );
  scale.set(transform.scale[0], transform.scale[1], transform.scale[2]);
}
