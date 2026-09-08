import { describe, expect, it } from "vitest";
import { ShaderLib } from "three";

/**
 * tint 着色器注入点存在性（5a 调试教训：onBeforeCompile 里的
 * `.replace("#include <...>")` 目标不存在时是静默无效，整个注入链
 * 无声消失。这里把注入目标锁进测试，three 升级时提前报警）。
 * 注：three r167+ 把 meshphysical 更名为 physical。
 */
describe("three physical(MeshStandard) 注入点", () => {
  const physical = ShaderLib.physical ?? ShaderLib.meshphysical;
  const fragment = physical.fragmentShader;
  const vertex = physical.vertexShader;

  it("fragment 注入点存在", () => {
    for (const chunk of [
      "common",
      "map_fragment",
      "normal_fragment_maps",
      "lights_fragment_end",
    ]) {
      expect(fragment).toContain(`#include <${chunk}>`);
    }
  });

  it("vertex 注入点存在", () => {
    for (const chunk of ["common", "uv_vertex", "beginnormal_vertex"]) {
      expect(vertex).toContain(`#include <${chunk}>`);
    }
  });
});
