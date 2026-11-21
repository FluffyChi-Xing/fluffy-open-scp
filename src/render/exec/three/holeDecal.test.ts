import * as THREE from "three";
import { expect, it } from "vitest";
import { createEngineDecalMaterial } from "@/lib/decalEngineMaterials";
import { configureHoleDecal } from "./holeDecal";
import { decalFrameObject } from "./decal";
import { decalFramePlan } from "../../plan/decal";

function fixture() {
  const material = createEngineDecalMaterial(THREE, "hole", {
    map: new THREE.Texture(), decalData: null,
    worldDirection: new THREE.Vector3(0, 0, 1),
    env: {
      sunDir: { value: new THREE.Vector3(0, 0, 1) },
      sunColor: { value: new THREE.Color(1, 1, 1) },
      dayLight: { value: 1 }, nightBoost: { value: 1 },
    },
  });
  const frame = decalFrameObject(THREE, decalFramePlan(
    [1, 0, 0, 0, 1, 0, 0, 0, 1, 10, 20, 30], 2, 1, 2,
  )!);
  configureHoleDecal(THREE, material, frame, 2);
  return material;
}

it("maps the authored full volume including its front half to the engine cube", () => {
  const material = fixture();
  const matrix = material.uniforms.uHoleToBox.value as THREE.Matrix4;
  expect(new THREE.Vector3(6, 18, 30).applyMatrix4(matrix).toArray()).toEqual([-1, -1, -1]);
  expect(new THREE.Vector3(14, 22, 32).applyMatrix4(matrix).toArray()).toEqual([1, 1, 1]);
  expect(material.fragmentShader).not.toContain("textureFloatPosition.z < 0.0");
  expect(material.fragmentShader).toContain("abs(vTexcoord0)");
  expect(material.fragmentShader).toContain("vTexcoord0 + ray * exitT");
  // Foreground stays on the receiver; only the interior uses the far cube.
  expect(material.fragmentShader).toContain("texture2D(uSampler0, vUv)");
  expect(material.fragmentShader).toContain("textureFloatPosition.xy * 0.5");
  material.dispose();
});

it("updates camera coordinates through transformed parent groups and orthographic views", () => {
  const material = fixture();
  const mesh = new THREE.Mesh(new THREE.BoxGeometry(), material);
  mesh.position.set(100, 200, 300);
  mesh.rotation.z = Math.PI / 2;
  mesh.updateMatrixWorld(true);
  const camera = new THREE.PerspectiveCamera();
  camera.position.copy(new THREE.Vector3(10, 20, 28).applyMatrix4(mesh.matrixWorld));
  camera.updateMatrixWorld(true);
  const render = (view: THREE.Camera) => material.onBeforeRender(
    {} as THREE.WebGLRenderer, new THREE.Scene(), view, mesh.geometry, mesh,
    {} as THREE.Group,
  );
  render(camera);
  const point = material.uniforms.uHoleCameraBox.value as THREE.Vector3;
  expect(point.x).toBeCloseTo(0);
  expect(point.y).toBeCloseTo(0);
  expect(point.z).toBeCloseTo(-3);
  expect(material.uniforms.uHoleOrthographic.value).toBe(false);
  const ortho = new THREE.OrthographicCamera();
  ortho.updateMatrixWorld(true);
  render(ortho);
  expect(material.uniforms.uHoleOrthographic.value).toBe(true);
  expect(material.uniforms.uHoleOrthoRay.value.length()).toBeCloseTo(1);
  mesh.geometry.dispose();
  material.dispose();
});
