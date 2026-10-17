import * as THREE from "three";
import type { Region3DData } from "@/lib/region-map";
import { cellRandom, fieldIndex } from "./map-fields";
import type { TreeAsset } from "./map-native-assets";

/** Density-driven preview trees; species and placements are not saved game instances. */
export function createVegetation(
  data: Region3DData,
  eco: ImageData,
  roadMask: Uint8Array,
  height: (x: number, y: number) => number,
  native: TreeAsset[] = [],
): THREE.Group {
  const group = new THREE.Group();
  group.name = "vegetation";
  const span = data.size * data.metersPerPixel;
  const step = 32;
  const canopyGeometry = new THREE.IcosahedronGeometry(1, 1);
  const distantCanopyGeometry = new THREE.IcosahedronGeometry(1, 0);
  const trunkGeometry = new THREE.CylinderGeometry(0.5, 0.8, 1, 5);
  const canopyMaterial = new THREE.MeshLambertMaterial({ color: "#517543" });
  const trunkMaterial = new THREE.MeshLambertMaterial({ color: "#63513a" });
  const transform = new THREE.Object3D();
  // Spatial batches allow frustum culling without one draw call per tree.
  for (let ty = 0; ty < span; ty += 2048) for (let tx = 0; tx < span; tx += 2048) {
    const positions: Array<[number, number, number, number]> = [];
    for (let y = ty; y < Math.min(ty + 2048, span); y += step) {
      for (let x = tx; x < Math.min(tx + 2048, span); x += step) {
        const gx = Math.floor((data.originWorld[0] + x) / step);
        const gy = Math.floor((data.originWorld[1] + y) / step);
        const wx = data.originWorld[0] + x + 4 + cellRandom(gx, gy, 1) * 24;
        const wy = data.originWorld[1] + y + 4 + cellRandom(gx, gy, 2) * 24;
        const i = fieldIndex(wx, wy, eco.width, data.originWorld, data.metersPerPixel);
        if (i < 0 || cellRandom(gx, gy, 3) > eco.data[i * 4 + 1] / 255) continue;
        const ix = i % eco.width, iy = Math.floor(i / eco.width);
        let nearRoad = false;
        for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) {
          if (ix + dx >= 0 && ix + dx < eco.width && iy + dy >= 0 && iy + dy < eco.height
            && roadMask[(iy + dy) * eco.width + ix + dx]) nearRoad = true;
        }
        const z = height(wx, wy);
        const slope = Math.hypot(height(wx + 16, wy) - height(wx - 16, wy),
          height(wx, wy + 16) - height(wx, wy - 16)) / 32;
        if (nearRoad || z < data.waterZ + 3 || slope > 0.65) continue;
        positions.push([wx, z, wy, 12 + cellRandom(gx, gy, 4) * 12]);
      }
    }
    if (!positions.length) continue;
    const centerX = data.originWorld[0] + Math.min(tx + 1024, span);
    const centerZ = data.originWorld[1] + Math.min(ty + 1024, span);
    const centerY = positions.reduce((sum, p) => sum + p[1], 0) / positions.length;
    if (native.length) {
      native.forEach((asset, species) => {
        const trees = positions.filter(([x,,z]) => Math.floor(cellRandom(Math.floor(x/step),Math.floor(z/step),7)*native.length) === species);
        if (!trees.length) return;
        const near = new THREE.InstancedMesh(asset.geometry, asset.material, trees.length);
        const far = new THREE.InstancedMesh(asset.farGeometry, asset.farMaterial, trees.length);
        trees.forEach(([x,y,z], i) => {
          const gx=Math.floor(x/step), gy=Math.floor(z/step);
          transform.position.set(x-centerX,y-centerY,z-centerZ);
          transform.rotation.set(0,cellRandom(gx,gy,8)*Math.PI*2,0);
          transform.scale.setScalar(0.7+cellRandom(gx,gy,9)*0.5);
          transform.updateMatrix(); near.setMatrixAt(i,transform.matrix);
        });
        far.instanceMatrix=near.instanceMatrix;
        near.computeBoundingSphere();far.computeBoundingSphere();
        const lod=new THREE.LOD();lod.position.set(centerX,centerY,centerZ);
        lod.addLevel(near,0);lod.addLevel(far,1800,0.15);group.add(lod);
      });
      continue;
    }
    const crowns = new THREE.InstancedMesh(canopyGeometry, canopyMaterial, positions.length);
    const trunks = new THREE.InstancedMesh(trunkGeometry, trunkMaterial, positions.length);
    positions.forEach(([x, y, z, h], i) => {
      transform.position.set(x - centerX, y - centerY + h * 0.67, z - centerZ);
      transform.scale.set(h * 0.35, h * 0.43, h * 0.35);
      transform.updateMatrix();
      crowns.setMatrixAt(i, transform.matrix);
      const gx = Math.floor(x / step), gy = Math.floor(z / step);
      crowns.setColorAt(i, new THREE.Color().setHSL(0.25 + cellRandom(gx, gy, 5) * 0.05, 0.26, 0.42 + cellRandom(gx, gy, 6) * 0.15));
      transform.position.set(x - centerX, y - centerY + h * 0.23, z - centerZ);
      transform.scale.set(1, h * 0.46, 1);
      transform.updateMatrix();
      trunks.setMatrixAt(i, transform.matrix);
    });
    crowns.computeBoundingSphere();
    trunks.computeBoundingSphere();
    const distantCrowns = new THREE.InstancedMesh(distantCanopyGeometry, canopyMaterial, positions.length);
    // Share instance attributes: LOD changes geometry, never placement/color or GPU instance storage.
    distantCrowns.instanceMatrix = crowns.instanceMatrix;
    distantCrowns.instanceColor = crowns.instanceColor;
    distantCrowns.computeBoundingSphere();
    const near = new THREE.Group();
    near.add(trunks, crowns);
    const lod = new THREE.LOD();
    lod.position.set(centerX, centerY, centerZ);
    lod.addLevel(near, 0);
    lod.addLevel(distantCrowns, 3500, 0.15);
    group.add(lod);
  }
  // The caller disposes shared geometries/materials once using a Set.
  if (!group.children.length || native.length) {
    canopyGeometry.dispose(); distantCanopyGeometry.dispose(); trunkGeometry.dispose(); canopyMaterial.dispose(); trunkMaterial.dispose();
  }
  return group;
}
