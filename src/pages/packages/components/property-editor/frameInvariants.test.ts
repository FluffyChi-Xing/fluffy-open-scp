import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { FRAMING_GROUPS, VIEWPORT_GROUPS } from "./useEditorViewport";
import { placementInverse, buildLotRect } from "./editorGround";

/**
 * 【关键不变量守卫 2026-10-27】「建筑飘在网格上方」回归(两轮、代价沉重)
 * 之后钉死的取景契约。本目录已冻结(见 FROZEN.md),改动本文件覆盖的任何
 * 行为前必须先读三条不变量:
 *  1. 网格/取景锚定「取景内容组」(FRAMING_GROUPS = model/lot/dimensions),
 *     单元 gizmo(灯/道具/贴花/特效/生成器/路径标记)永不参与;
 *  2. placement 矩阵是 WPF 行向量约定(平移在 12 浮点第 4 行),换算到
 *     three 列向量必须转置——黄金用例取自 2026-10-27 真实 lot 取证数据;
 *  3. 地面矩形贴 lot 局部 z≈0 平面(地面网格锚定的参照面)。
 * 另:装配尾部 scene.updateMatrixWorld(true)(PropertyEditorViewport)与
 * 软跟随 frameContent(three-viewer)同为冻结不变量,由代码注释标记。
 */

describe("取景不变量:网格/取景锚定内容组,gizmo 永不参与", () => {
  it("FRAMING_GROUPS 覆盖内容三组且排除全部 gizmo 组", () => {
    for (const name of ["model", "lot", "dimensions"] as const) {
      expect(FRAMING_GROUPS).toContain(name);
    }
    for (const group of VIEWPORT_GROUPS) {
      if (!(FRAMING_GROUPS as readonly string[]).includes(group)) {
        // 该组为编辑器 chrome:必须保持缺席
        expect(FRAMING_GROUPS).not.toContain(group);
      }
    }
    // gizmo 组数量守恒:若有人新增图层组,必须显式决定它是否参与取景
    expect(VIEWPORT_GROUPS).toHaveLength(9);
  });
});

describe("placement 矩阵不变量:WPF 行向量 → three 列向量", () => {
  it("黄金用例(2026-10-27 真实 lot 取证):placement 位移 (0,9,0) → 地面位移 (0,-9,0)", () => {
    const placement = [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 9, 0];
    const matrix = placementInverse(THREE, placement);
    const elements = matrix.elements; // three 列主序
    // 旋转部分仍为单位阵
    expect(elements[0]).toBeCloseTo(1);
    expect(elements[5]).toBeCloseTo(1);
    expect(elements[10]).toBeCloseTo(1);
    // 平移在第 4 列 = (0,-9,0)
    expect(elements[12]).toBeCloseTo(0);
    expect(elements[13]).toBeCloseTo(-9);
    expect(elements[14]).toBeCloseTo(0);
  });

  it("buildLotRect 贴 lot 局部 z≈0 平面(地面网格锚定的参照面)", () => {
    const ground = buildLotRect(THREE, [48, 34]);
    ground.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(ground);
    // 边框线 z=0.05、填充面 z=0.02:整个地面紧贴 z=0 平面
    expect(box.min.z).toBeGreaterThanOrEqual(-0.01);
    expect(box.max.z).toBeLessThan(0.2);
  });
});
