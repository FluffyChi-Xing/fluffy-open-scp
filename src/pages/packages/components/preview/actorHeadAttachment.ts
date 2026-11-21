import { Matrix4, Vector3 } from "three";
/**
 * Native cVignettePool::UpdateVignettePool (0x7DEA60, source lines 668–720):
 * transform the bind-pose head point through the animated bone, but keep
 * the actor's rotation/scale. A head is not a fully parented bone child.
 * Both matrices include ThreeViewer's Z-up→Y-up root; removing bodyWorld
 * converts the animated attachment point back into the body's Z-up space.
 */
export function actorHeadPosition(
  bodyWorld: Matrix4,
  boneWorld: Matrix4,
): Vector3 {
  const local = new Matrix4().copy(bodyWorld).invert().multiply(boneWorld);
  return new Vector3().setFromMatrixPosition(local);
}
