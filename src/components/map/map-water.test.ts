import { describe, expect, it } from "vitest";
import type { Region3DData } from "@/lib/region-map";
import { createMapWater } from "./map-water";

const data = { size: 2, metersPerPixel: 16, originWorld: [-16, -16], waterZ: 20,
  heightDiv: 32, heightBias: -1024 } as Region3DData;
const pixels = { width: 2, height: 2, data: new Uint8ClampedArray(16) } as ImageData;

describe("region water", () => {
  it("honors native timing including paused water and zero specular intensity", () => {
    const water = createMapWater({ ...data, waterParams: { timeStepFactor: 0, specularPower: 300, specularScale: 0 } }, pixels);
    water.update(10);
    expect(water.mesh.material.uniforms.time.value).toBe(0);
    expect(water.mesh.material.uniforms.specularScale.value).toBe(0);
    expect(water.mesh.material.uniforms.specularPower.value).toBe(300);
    const moving = createMapWater({ ...data, waterParams: { timeStepFactor: 2, specularPower: null, specularScale: null } }, pixels);
    moving.update(10);
    expect(moving.mesh.material.uniforms.time.value).toBe(20);
    water.mesh.material.dispose(); moving.mesh.material.dispose();
  });

  it("uses finite fallback parameters and releases all owned textures", () => {
    const water = createMapWater({ ...data, waterParams: { timeStepFactor: NaN, specularPower: -1, specularScale: Infinity } }, pixels);
    water.update(1);
    expect(water.mesh.material.uniforms.time.value).toBe(5);
    expect(water.mesh.material.uniforms.specularPower.value).toBe(500);
    expect(water.mesh.material.uniforms.specularScale.value).toBe(11);
    const released: string[] = [];
    for (const key of ["heightMap", "normalMap", "foamMap"]) {
      water.mesh.material.uniforms[key].value.addEventListener("dispose", () => released.push(key));
    }
    water.mesh.material.dispose();
    expect(released.sort()).toEqual(["foamMap", "heightMap", "normalMap"]);
  });
});
