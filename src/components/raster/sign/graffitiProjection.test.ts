import * as THREE from "three";
import { expect, it } from "vitest";
import { RasterDocument } from "@/lib/raster-editor/document";
import { createSunEnv, applySunEnv } from "@/pages/packages/components/property-editor/refinedRender";
import { createGraffitiProjection } from "./graffitiProjection";

it("projects an unflipped raw paint mask with the Property Editor material and environment", async () => {
  const doc = new RasterDocument(2, 2, Uint8ClampedArray.of(
    180, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 220,
  ));
  const projection = await createGraffitiProjection(
    THREE, doc, ["#ff0000", "#00ff00", "#0000ff", "#ffffff"], 1, 1, () => false,
  );
  expect(projection).not.toBeNull();
  const { scene, setTime, updatePixels, dispose } = projection!;
  try {
    scene.updateMatrixWorld(true);
    const meshes: THREE.Mesh[] = [];
    scene.traverse((node) => { if (node instanceof THREE.Mesh && node.material instanceof THREE.ShaderMaterial) meshes.push(node); });
    expect(meshes).toHaveLength(1);
    const mesh = meshes[0];
    const shader = mesh.material as THREE.ShaderMaterial;
    expect(shader.uniforms.uGraffiti.value).toBe(1);
    const uv = mesh.geometry.attributes.uv;
    const positions = mesh.geometry.attributes.position;
    for (let index = 0; index < positions.count; index++) {
      expect(uv.getX(index)).toBeCloseTo((positions.getX(index) + 1) / 2);
      expect(uv.getY(index)).toBeCloseTo((positions.getY(index) + 1) / 2);
    }
    const map = shader.uniforms.uSampler0.value as THREE.DataTexture;
    const data = map.image.data;
    if (!data) throw new Error("Projected graffiti has no texture data");
    expect(map.minFilter).toBe(THREE.LinearMipmapLinearFilter);
    expect(Array.from(data)).toEqual([0, 0, 0, 0, 0, 0, 0, 220, 180, 0, 0, 0, 0, 0, 0, 0]);
    doc.pixels[0] = 195;
    updatePixels(doc);
    expect(data[8]).toBe(195);
    const peEnv = createSunEnv(THREE);
    for (const time of [0, 6, 12]) {
      applySunEnv(peEnv, time, true);
      setTime(time);
      expect(shader.uniforms.uNightBoost.value).toBe(peEnv.nightBoost.value);
    }
  } finally {
    dispose();
  }
});
