import { describe, expect, it } from "vitest";
import { classifyProp } from "./propSemantic";

const treeIds = new Set([0x4df43690]);

describe("classifyProp（bin 只是绘制组，类型来自资源）", () => {
  it("treeIds 命中优先", () => {
    expect(classifyProp({ resourceId: 0x4df43690, treeIds })).toBe("tree");
  });

  it("已验证 resourceId 映射", () => {
    expect(classifyProp({ resourceId: 0x0ab4dfe1, treeIds })).toBe("vehicle");
    expect(classifyProp({ resourceId: 0x85271637, treeIds })).toBe("waste");
  });

  it("名称回退与未知资源", () => {
    expect(classifyProp({ resourceId: null, name: "For Sale Sign", treeIds })).toBe("sign");
    expect(classifyProp({ resourceId: null, name: "cargo crate", treeIds })).toBe("product");
    expect(classifyProp({ resourceId: 0x12345678, treeIds })).toBe("static");
  });

  it("bin 不是分类依据：同一资源在不同 bin 结果一致", () => {
    const facts = { resourceId: 0x0ab4dfe1, treeIds };
    expect(classifyProp(facts)).toBe(classifyProp(facts));
  });
});
