import * as THREE from "three";

/** Game right-handed Z-up -> Three right-handed Y-up: a rotation, not an axis swap. */
export function mapToScene(x: number, y: number, height: number): THREE.Vector3 {
  return new THREE.Vector3(x, height, -y);
}

export function sceneToMap(position: THREE.Vector3): THREE.Vector3 {
  return new THREE.Vector3(position.x, -position.z, position.y);
}

/** Layer builders use map-local (x, height, y) coordinates. Apply this boundary
 * once to ALL layers. Three handles winding/normal transforms for this group.
 * Data fields and editor coordinates remain in the original game coordinate system.
 */
export function createMapSceneRoot(): THREE.Group {
  const root = new THREE.Group();
  root.name = "map-world";
  root.scale.z = -1;
  return root;
}

/** Inverse of the map root for data-texture lookup in world-space shaders. */
export const MAP_WORLD_XY_GLSL = `
vec2 mapWorldXY(vec3 scenePosition) { return vec2(scenePosition.x, -scenePosition.z); }
`;
