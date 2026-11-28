import * as THREE from "three";
import type { Region3DData } from "@/lib/region-map";
type Assets = NonNullable<Region3DData["roadAssets"]>;
/**
 * Inspector curves have no terrain context. Bridge components mix path-relative
 * columns with terrain-relative footings (BuildInstancedModels, 0x7AB190, lines 121–155).
 * Put the example terrain below the fixed road structure, rather than putting
 * both coordinate anchors at zero. Never modify resource vertices or offsets.
 */
export function roadPreviewGroundHeight(
  assets: Assets,
  instance: number,
): number {
  const sweeps = assets.sweeps[instance] ?? [];
  if (!sweeps.some((s) => s.stackToGround && !s.relativeToGround)) return 0;
  let roadBottom = Infinity,
    groundTop = -Infinity,
    stackTop = Infinity;
  for (const s of sweeps) {
    const model = assets.models[s.model];
    if (!model) continue;
    const rotation = new THREE.Euler(
      ...(s.rotation.map((v) => (v * Math.PI) / 180) as [
        number,
        number,
        number,
      ]),
      "ZYX",
    );
    const transform = new THREE.Matrix4().compose(
      new THREE.Vector3(),
      new THREE.Quaternion().setFromEuler(rotation),
      s.instance
        ? new THREE.Vector3().setScalar(s.scale[0])
        : new THREE.Vector3(...s.scale),
    );
    let min = Infinity,
      max = -Infinity;
    for (let i = 0; i < model.positions.length; i += 3) {
      const p = new THREE.Vector3(
        model.positions[i],
        model.positions[i + 1],
        model.positions[i + 2],
      ).applyMatrix4(transform);
      min = Math.min(min, p.z + s.offset[2]);
      max = Math.max(max, p.z + s.offset[2]);
    }
    if (s.relativeToGround) groundTop = Math.max(groundTop, max);
    else if (!s.stackToGround) roadBottom = Math.min(roadBottom, min);
    else stackTop = Math.min(stackTop, max);
  }
  const anchor = Number.isFinite(roadBottom) ? roadBottom : stackTop;
  return Number.isFinite(anchor) && Number.isFinite(groundTop)
    ? Math.min(0, anchor - groundTop)
    : 0;
}
