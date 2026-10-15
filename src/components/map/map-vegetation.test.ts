import { describe, expect, it } from "vitest";
import * as THREE from "three";
import type { Region3DData } from "@/lib/region-map";
import { createVegetation } from "./map-vegetation";

const data = { size: 16, metersPerPixel: 16, originWorld: [-128, -128], waterZ: 0 } as Region3DData;
const eco = { width: 16, height: 16, data: new Uint8ClampedArray(16 * 16 * 4).fill(255) } as ImageData;

describe("forest preview LOD", () => {
  it("preserves tree placement and colors while reducing distant geometry by 80%", () => {
    const forest = createVegetation(data, eco, new Uint8Array(256), () => 20);
    const lod = forest.children[0] as THREE.LOD;
    const near = lod.levels[0].object as THREE.Group;
    const crowns = near.children[1] as THREE.InstancedMesh;
    const distant = lod.levels[1].object as THREE.InstancedMesh;
    expect(distant.instanceMatrix).toBe(crowns.instanceMatrix);
    expect(distant.instanceColor).toBe(crowns.instanceColor);
    const triangles = (mesh: THREE.InstancedMesh) =>
      (mesh.geometry.index?.count ?? mesh.geometry.getAttribute("position").count) / 3 * mesh.count;
    const nearTriangles = near.children.reduce((sum, mesh) => sum + triangles(mesh as THREE.InstancedMesh), 0);
    expect(triangles(distant)).toBeLessThanOrEqual(nearTriangles * 0.2);
    forest.updateMatrixWorld(true);
    const camera = new THREE.PerspectiveCamera();
    camera.position.copy(lod.position).add(new THREE.Vector3(0, 1000, 0));
    camera.updateMatrixWorld(); lod.update(camera);
    expect(near.visible).toBe(true);
    camera.position.y += 10000;
    camera.updateMatrixWorld(); lod.update(camera);
    expect(near.visible).toBe(false);
    expect(distant.visible).toBe(true);
    const matrix = new THREE.Matrix4();
    crowns.getMatrixAt(0, matrix);
    const position = new THREE.Vector3().setFromMatrixPosition(matrix).applyMatrix4(crowns.matrixWorld);
    expect(position.x).toBeGreaterThanOrEqual(-128);
    expect(position.x).toBeLessThan(128);
    expect(position.z).toBeGreaterThanOrEqual(-128);
    expect(position.z).toBeLessThan(128);
    expect(position.y).toBeGreaterThan(20);
  });

  it("does not place trees underwater or in a cleared forest field", () => {
    expect(createVegetation(data, eco, new Uint8Array(256), () => -1).children).toHaveLength(0);
    const empty = { ...eco, data: new Uint8ClampedArray(16 * 16 * 4) };
    expect(createVegetation(data, empty, new Uint8Array(256), () => 20).children).toHaveLength(0);
  });
});
