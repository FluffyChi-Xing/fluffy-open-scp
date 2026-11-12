import type * as ThreeNamespace from "three";

export function createLineLightMarker(
  THREE: typeof ThreeNamespace,
  color: ThreeNamespace.Color,
  length: number,
): ThreeNamespace.Mesh {
  const geometry = new THREE.BoxGeometry(1, length, 1);
  geometry.translate(0, length / 2, 0);
  const marker = new THREE.Mesh(
    geometry,
    new THREE.MeshBasicMaterial({ color, toneMapped: false }),
  );
  marker.name = "__lineLightEmitter";
  marker.userData.pickOnly = true;
  return marker;
}
