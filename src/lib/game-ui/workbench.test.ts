import { describe, expect, it } from "vitest";
import {
  applyEdit,
  assetPathForRef,
  fnv1Lower,
  sortTools,
} from "@/lib/game-ui/workbench";

describe("fnv1Lower", () => {
  it("与游戏资源命名一致（locale 表 gameentry → 0xB844F811 实测向量）", () => {
    expect(fnv1Lower("gameentry").toString(16).toUpperCase()).toBe("B844F811");
  });
});

describe("assetPathForRef", () => {
  it("把布局树图片引用解析到提取目录", () => {
    expect(assetPathForRef("Graphics/HUD/puck-base.png")).toBe(
      "/game-ui/png/00000000_7AF20E3C.png",
    );
    expect(assetPathForRef("Graphics/HUD/main_button_tab_mid.png")).toBe(
      "/game-ui/png/00000000_3E12FFEA.png",
    );
  });

  it("非图片引用返回 null", () => {
    expect(assetPathForRef("not-an-image")).toBeNull();
    expect(assetPathForRef("weird.txt")).toBeNull();
  });
});

describe("applyEdit / sortTools", () => {
  const base = {
    id: "AAAA0001",
    label: "宿舍",
    pos: 100,
    icon: null,
    source: "locale" as const,
  };

  it("applyEdit 只覆盖给出的字段", () => {
    expect(applyEdit(base, undefined)).toEqual(base);
    expect(applyEdit(base, { label: "我的宿舍" })).toEqual({
      ...base,
      label: "我的宿舍",
    });
    expect(applyEdit(base, { icon: "/game-ui/png/x.png" }).icon).toBe(
      "/game-ui/png/x.png",
    );
  });

  it("sortTools 按 pos 稳定排序", () => {
    const tools = [
      { ...base, id: "B", pos: 20 },
      { ...base, id: "A", pos: 10 },
      { ...base, id: "C", pos: 20 },
    ];
    expect(sortTools(tools).map((tool) => tool.id)).toEqual(["A", "B", "C"]);
  });
});
