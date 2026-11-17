import * as THREE from "three";
import { windArrowRotation, type Region3DData } from "@/lib/region-map";

const arrowLength = 420;

export function createWindArrows(capacity: number, height: (x: number, y: number) => number, waterZ: number) {
  const shape = new THREE.Shape();
  shape.moveTo(-210, -25);
  shape.lineTo(64, -25);
  shape.lineTo(64, -75);
  shape.lineTo(210, 0);
  shape.lineTo(64, 75);
  shape.lineTo(64, 25);
  shape.lineTo(-210, 25);
  shape.closePath();
  const geometry = new THREE.ShapeGeometry(shape).rotateX(Math.PI / 2);
  const material = new THREE.MeshBasicMaterial({
    color: 0x68e2ff, side: THREE.DoubleSide, depthTest: false, depthWrite: false,
  });
  const layer = new THREE.Group();
  layer.name = "city-wind-arrows";
  layer.visible = false;
  for (let i = 0; i < capacity; i++) {
    const arrow = new THREE.Mesh(geometry, material);
    arrow.renderOrder = 8;
    arrow.visible = false;
    layer.add(arrow);
  }
  if (!capacity) {
    geometry.dispose();
    material.dispose();
  }

  function update(arrows: Region3DData["windArrows"], visible: boolean) {
    layer.visible = visible;
    if (!visible) return;
    layer.children.forEach((arrow, i) => {
      const reading = arrows?.[i];
      arrow.visible = !!reading;
      if (!reading) return;
      const { x, y, direction } = reading;
      arrow.position.set(x, Math.max(height(x, y), waterZ) + 40, y);
      arrow.rotation.y = THREE.MathUtils.degToRad(windArrowRotation(direction));
    });
  }
  const worldPosition = new THREE.Vector3();
  function sizeForView(camera: THREE.PerspectiveCamera, viewportHeight: number) {
    if (!layer.visible || viewportHeight <= 0) return;
    const viewHeight = 2 * Math.tan(THREE.MathUtils.degToRad(camera.fov) / 2);
    for (const arrow of layer.children) {
      if (!arrow.visible) continue;
      arrow.getWorldPosition(worldPosition);
      const distance = camera.position.distanceTo(worldPosition);
      const pixels = arrowLength * viewportHeight / (viewHeight * Math.max(distance, 1));
      const targetPixels = THREE.MathUtils.clamp(pixels, 26, 56);
      arrow.scale.setScalar(THREE.MathUtils.clamp(targetPixels / pixels, 0.3, 4));
    }
  }
  return { layer, update, sizeForView };
}
