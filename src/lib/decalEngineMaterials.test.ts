import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { createEngineDecalMaterial, type EngineMaterialOptions } from "./decalEngineMaterials";

describe("neon pass weights", () => {
  it("restores authored projected light weights without doubling animation parameters", () => {
    const options: EngineMaterialOptions = {
      map: new THREE.Texture(),
      layerColors: [[2, 4, 6, -120], [4, 6, 8, 0.6], [6, 8, 10, 1.2], [8, 10, 12, 1.8]],
      decalData: null,
      worldDirection: new THREE.Vector3(0, 0, 1),
      env: {
        sunDir: { value: new THREE.Vector3(0, 0, 1) },
        sunColor: { value: new THREE.Color(1, 1, 1) },
        dayLight: { value: 1 },
        nightBoost: { value: 1 },
      },
    };
    const light = createEngineDecalMaterial(THREE, "neon-light", options);
    const tube = createEngineDecalMaterial(THREE, "sdf", options);
    expect(light.uniforms.uDecalMaterialData.value[0].toArray()).toEqual([1, 2, 3, 4]);
    expect(tube.uniforms.uDecalMaterialData.value[0].toArray()).toEqual([2, 4, 6, 8]);
    expect(light.uniforms.uDecalMaterialData.value[3].toArray()).toEqual([-60, 0.3, 0.6, 0.9]);
    expect(tube.uniforms.uDecalMaterialData.value[3].toArray()).toEqual([-60, 0.3, 0.6, 0.9]);
    light.dispose();
    tube.dispose();
    options.map.dispose();
  });
});
