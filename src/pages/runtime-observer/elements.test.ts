import { describe, expect, it } from "vitest";
import { elementModel } from "./elements";
describe("element hierarchy", () => {
  it("preserves hidden ancestor metadata and overlays visible bounds", () => {
    const tree = elementModel({
      pid: 1,
      time: 1,
      sequence: 1,
      viewport: [100, 100],
      nodes: [
        { i: 0, parent: -1, tag: "html" },
        { i: 1, parent: 0, tag: "div", classes: "SystemInfo_SubWindow" },
      ],
      boxes: [
        {
          i: 1,
          parent: 0,
          tag: "div",
          rect: [1, 2, 40, 50],
          text: "Statistics",
        },
      ],
    });
    expect(tree.children.get(0)?.[0]).toMatchObject({
      classes: "SystemInfo_SubWindow",
      text: "Statistics",
    });
    expect(tree.children.get(-1)).toHaveLength(1);
  });
  it("supports older bounds-only frames without inventing missing parents", () => {
    const tree = elementModel({
      pid: 1,
      time: 1,
      sequence: 1,
      viewport: [100, 100],
      boxes: [{ i: 5, parent: 4, tag: "div" }],
    });
    expect(tree.children.get(-1)?.[0].i).toBe(5);
  });
});
