import { describe, expect, it } from "vitest";
import {
  initOffsets,
  PIN_CENTER,
  PIN_FILL,
  PIN_LEFT,
  PIN_PROPORTIONAL,
  PIN_RIGHT,
  PIN_STRETCH,
  updatePosition,
  type LayoutNode,
} from "@/lib/game-ui/scrui";

function node(partial: Partial<LayoutNode>): LayoutNode {
  return { ...partial };
}

/** 引擎两段式：设计尺寸 Init → 运行尺寸 Update。 */
function place(n: Partial<LayoutNode>, designP: [number, number], runP: [number, number]) {
  const full = node(n);
  initOffsets(full, designP[0], designP[1]);
  return updatePosition(full, runP[0], runP[1]);
}

describe("scrui pin semantics（语义逆向自 7D14BE70.js，公式为引擎原文）", () => {
  it("PIN_LEFT：绝对定位，不随父尺寸变化", () => {
    const r = place(
      { left: 40, top: 30, width: 100, height: 20, horizontalPinType: PIN_LEFT, verticalPinType: PIN_LEFT },
      [1024, 793],
      [1600, 900],
    );
    expect(r).toEqual({ x: 40, y: 30, w: 100, h: 20 });
  });

  it("PIN_RIGHT：到父『末边』的距离固定（main stats 实测锚点）", () => {
    // main stats：authored (118.5, 822, 847×35)，vpin=Right，父 793→900
    const r = place(
      { left: 118.5, top: 822, width: 847, height: 35, verticalPinType: PIN_RIGHT },
      [1024, 793],
      [1600, 900],
    );
    expect(r.y).toBe(929); // 822 + (900 - 793)
    expect(r.h).toBe(35);
  });

  it("PIN_STRETCH：锚边固定、尺寸随父增长（main stats 横向 847→1423）", () => {
    const r = place(
      { left: 118.5, top: 0, width: 847, height: 35, horizontalPinType: PIN_STRETCH },
      [1024, 793],
      [1600, 900],
    );
    expect(r.x).toBe(118.5);
    expect(r.w).toBeCloseTo(1423); // 847 + (1600 - 1024)
  });

  it("PIN_CENTER：中心点随父中心平移", () => {
    const r = place(
      { left: 462, top: 0, width: 100, height: 10, horizontalPinType: PIN_CENTER },
      [1024, 793],
      [1600, 900],
    );
    expect(r.x).toBeCloseTo(462 + 288); // (1600-1024)/2
  });

  it("PIN_FILL：铺满父容器", () => {
    const r = place(
      { width: 10, height: 10, horizontalPinType: PIN_FILL, verticalPinType: PIN_FILL },
      [1024, 793],
      [1600, 900],
    );
    expect(r).toEqual({ x: 0, y: 0, w: 1600, h: 900 });
  });

  it("PIN_PROPORTIONAL：两端按比例锚定", () => {
    const r = place(
      {
        left: 100,
        width: 200,
        height: 10,
        horizontalPinType: PIN_PROPORTIONAL,
        leftProportionalRatio: 0.1,
        rightProportionalRatio: 0.3,
      },
      [1000, 793],
      [2000, 793],
    );
    expect(r.x).toBeCloseTo(200); // 0.1 * 2000
    expect(r.w).toBeCloseTo(600 - 200); // 0.3*2000 - x
  });
});
