import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { readFileSync } from "node:fs";
import { createEngineDecalMaterial } from "./decalEngineMaterials";

const env = {
  sunDir: { value: new THREE.Vector3(0, 0, 1) },
  sunColor: { value: new THREE.Color(1, 1, 1) },
  dayLight: { value: 1 },
};

describe("破洞体积材质", () => {
  it("向视口提供体积尺寸 uniform，避免读取 uBoxHalfXY.value 时崩溃", () => {
    const material = createEngineDecalMaterial(THREE, "hole", {
      map: new THREE.Texture(),
      decalData: [0.25, 1],
      worldDirection: new THREE.Vector3(0, 0, 1),
      env,
    });
    expect(material.uniforms.uBoxHalfXY?.value).toBeInstanceOf(THREE.Vector2);
    expect(material.uniforms.uHalfDepth?.value).toBeGreaterThan(0);
    expect(material.uniforms.uSunColor3).toBe(env.sunColor);
    material.dispose();
  });
  it("使用调用方盒体尺寸，并让生成的顶点着色器与 Rust 源保持一致", () => {
    const material = createEngineDecalMaterial(THREE, "hole", {
      map: new THREE.Texture(),
      decalData: null,
      worldDirection: new THREE.Vector3(0, 0, 1),
      env,
      boxHalfSize: [3, 2, 0.4],
    });
    expect(material.uniforms.uBoxHalfXY.value.toArray()).toEqual([3, 2]);
    expect(material.uniforms.uHalfDepth.value).toBe(0.4);
    const generator = readFileSync(
      "crates/sc-shader/src/compose.rs",
      "utf8",
    ).replace(/\r\n/g, "\n");
    expect(material.vertexShader.replace(/\r\n/g, "\n")).toBe(
      generator.match(/pub const HOLE_VS: &str = r#"([\s\S]*?)"#;/)?.[1],
    );
    material.dispose();
  });
});
