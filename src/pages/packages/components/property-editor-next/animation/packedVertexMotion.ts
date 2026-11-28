import { Vector3, type BufferAttribute, type BufferGeometry } from "three";

/** SimCity_App g40212002: unpackAxisAnim / unpackTransAngle / animMorphRot. */
export function createPackedVertexSampler(geometry: BufferGeometry, packed: ArrayBuffer, slot: number) {
  const position = geometry.getAttribute("position") as BufferAttribute;
  if (packed.byteLength !== position.count * 12) throw new Error("Packed motion vertex count mismatch");
  const bytes = new DataView(packed);
  const normal = geometry.getAttribute("normal") as BufferAttribute | undefined;
  const tangent = geometry.getAttribute("tangent") as BufferAttribute | undefined;
  const vertices: {
    i: number;
    axis: Vector3;
    pivot: Vector3;
    halfAngle: number;
    translation: boolean;
    position: Vector3;
    normal?: Vector3;
    tangent?: Vector3;
  }[] = [];
  for (let i = 0; i < position.count; i++) {
    const j = i * 12;
    if (bytes.getUint8(j + 3) !== slot) continue;
    const axis = new Vector3(
      bytes.getUint8(j) / 127 - 1,
      bytes.getUint8(j + 1) / 127 - 1,
      bytes.getUint8(j + 2) / 127 - 1,
    );
    const pivot = new Vector3(
      bytes.getInt16(j + 4, true) / 64,
      bytes.getInt16(j + 6, true) / 64,
      bytes.getInt16(j + 8, true) / 64,
    );
    const halfAngle = bytes.getInt16(j + 10, true) * (16 / 32768);
    const translation = axis.lengthSq() < 0.5;
    if (translation ? pivot.lengthSq() === 0 : halfAngle === 0) continue;
    vertices.push({
      i, axis, pivot, halfAngle, translation,
      position: new Vector3().fromBufferAttribute(position, i),
      normal: normal && new Vector3().fromBufferAttribute(normal, i),
      tangent: tangent && new Vector3().fromBufferAttribute(tangent, i),
    });
  }
  if (!vertices.length) return null;
  const temp = new Vector3();
  const q = new Vector3();
  const cross = new Vector3();
  const rotated = new Vector3();
  // HLSL rotateQ uses the decoded axis as-is, without normalizing it.
  function rotate(value: Vector3, qw: number) {
    cross.crossVectors(q, value).addScaledVector(value, qw);
    return value.add(rotated.crossVectors(q, cross));
  }
  return (time: number) => {
    for (const v of vertices) {
      temp.copy(v.position);
      if (v.translation) {
        temp.addScaledVector(v.pivot, time);
      } else {
        const halfTheta = v.halfAngle * time;
        q.copy(v.axis).multiplyScalar(Math.SQRT2 * Math.sin(halfTheta));
        const qw = Math.SQRT2 * Math.cos(halfTheta);
        rotate(temp.sub(v.pivot), qw).add(v.pivot);
        position.setXYZ(v.i, temp.x, temp.y, temp.z);
        if (normal && v.normal) {
          rotate(temp.copy(v.normal), qw);
          normal.setXYZ(v.i, temp.x, temp.y, temp.z);
        }
        if (tangent && v.tangent) {
          rotate(temp.copy(v.tangent), qw);
          tangent.setXYZ(v.i, temp.x, temp.y, temp.z);
        }
        continue;
      }
      position.setXYZ(v.i, temp.x, temp.y, temp.z);
    }
    position.needsUpdate = true;
    if (normal) normal.needsUpdate = true;
    if (tangent) tangent.needsUpdate = true;
  };
}
