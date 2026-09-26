import { readFileSync } from "node:fs";
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

/**
 * 注入 GLSL 的 `sc*` 标识符审计（2026-09-26 教训：移除下向面豁免时删掉了
 * `scExempt` 声明，但 lights_fragment_end 块仍在引用——GLSL 编译失败 ⇒
 * **整个 tint 材质静默不可见**（模型消失），而注入点测试全绿）。
 * 规则：注入块里出现的每个 `sc` 前缀标识符必须在注入块集合内声明
 * （uniform/varying/局部/函数）。three 自身 chunk 的变量不带 sc 前缀。
 */
describe("tint 注入 GLSL sc* 标识符审计", () => {
  const source = readFileSync(
    "src/pages/packages/components/property-editor/refinedRender.ts",
    "utf8",
  );
  // 模板字符串即注入块（判定：包含 #include 或 GLSL 类型关键字）。
  const chunks = [...source.matchAll(/`([^`]*)`/gs)]
    .map((m) => m[1])
    .filter((s) => s.includes("#include") || /\b(?:vec[234]|float|mat[234])\s+\w/.test(s));
  it("抽取到注入块（防正则失效静默通过）", () => {
    expect(chunks.length).toBeGreaterThanOrEqual(4);
  });

  const glsl = chunks
    .join("\n")
    // 剥掉行注释再审计（注释里提到的标识符不参与检查）。
    .replace(/\/\/[^\n]*/g, "");
  const declared = new Set<string>();
  // 局部/全局声明：float|vec|int|mat scXxx =
  for (const m of glsl.matchAll(/\b(?:float|double|vec2|vec3|vec4|ivec2|ivec3|ivec4|mat2|mat3|mat4)\s+(sc\w+)/g)) {
    declared.add(m[1]);
  }
  // 函数定义：vec3 scFoo(
  for (const m of glsl.matchAll(/\b(?:float|vec2|vec3|vec4)\s+(sc\w+)\s*\(/g)) {
    declared.add(m[1]);
  }
  // uniform / varying / attribute / define
  for (const m of glsl.matchAll(/\b(?:uniform|varying|attribute)\s+\w+\s+(\w+)/g)) {
    declared.add(m[1]);
  }
  for (const m of glsl.matchAll(/#\s*(?:if|elif)\s+defined?\(\s*(\w+)/g)) {
    declared.add(m[1]);
  }

  it("所有被引用的 sc* 标识符均已声明", () => {
    const used = new Set([...glsl.matchAll(/\b(sc[A-Z]\w*)\b/g)].map((m) => m[1]));
    const undeclared = [...used].filter((name) => !declared.has(name));
    expect(undeclared).toEqual([]);
  });
});
