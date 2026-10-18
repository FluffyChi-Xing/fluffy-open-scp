import { describe, expect, it } from "vitest";
import {
  generateBuildingParts,
  normalizeParameters,
} from "./generator";
import { DEFAULT_PARAMETERS } from "./schema";

const BAY = 3;

describe("procedural building generator", () => {
  it("is deterministic for the same parameters and seed", () => {
    const a = generateBuildingParts(
      { baysX: 4, baysY: 3, floors: 4, detail: "random" },
      42,
    );
    const b = generateBuildingParts(
      { baysX: 4, baysY: 3, floors: 4, detail: "random" },
      42,
    );
    expect(a).toEqual(b);
  });

  it("does not model windows or doors（窗户由立面贴图表达）", () => {
    const parts = generateBuildingParts(
      { baysX: 5, baysY: 3, floors: 3, balconies: "all", detail: "uniform" },
      1,
    );
    const kinds = new Set<string>(parts.map(part => part.kind));
    expect(kinds.has("windowGlass")).toBe(false);
    expect(kinds.has("windowFrame")).toBe(false);
    expect(kinds.has("storefront")).toBe(false);
    expect(kinds.has("door")).toBe(false);
    expect(kinds.has("sill")).toBe(false);
    // 阳台是出挑体量，保留几何
    expect(kinds.has("balconySlab")).toBe(true);
  });

  it("emits one full-height wall panel per side（立面图整贴单元）", () => {
    const floors = 3;
    const parts = generateBuildingParts(
      { baysX: 4, baysY: 2, floors, balconies: "none", detail: "off" },
      7,
    );
    const walls = parts.filter(part => part.kind === "wall");
    expect(walls).toHaveLength(4);
    // 全高：从地面到檐口底
    const fullHeight =
      DEFAULT_PARAMETERS.groundFloorHeight + floors * DEFAULT_PARAMETERS.floorHeight;
    for (const wall of walls) {
      expect(wall.size[2]).toBeCloseTo(fullHeight, 5);
    }
    // 槽位只落在 4 个材质槽
    const slots = new Set(parts.map(part => part.slot));
    for (const slot of slots) {
      expect(["wall", "trim", "roof", "ground"]).toContain(slot);
    }
  });

  it("keeps featured balconies to the first and top floors", () => {
    const floors = 4;
    const parts = generateBuildingParts(
      { baysX: 3, baysY: 3, floors, balconies: "featured", detail: "off" },
      1,
    );
    const slabs = parts.filter(part => part.kind === "balconySlab");
    // 前后 3 开间 + 左右 3 开间 = 12 扇/层，共 2 层
    expect(slabs).toHaveLength(12 * 2);
    const levels = new Set(
      slabs.map(part => Math.round(part.center[2] * 100)),
    );
    expect(levels).toHaveLength(2);
  });

  it("builds a taller mansard roof than a flat roof", () => {
    const mansard = generateBuildingParts(
      { roofStyle: "mansard", detail: "off", balconies: "none" },
      1,
    );
    const flat = generateBuildingParts(
      { roofStyle: "flat", detail: "off", balconies: "none" },
      1,
    );
    const roofTop = (parts: ReturnType<typeof generateBuildingParts>) =>
      Math.max(
        ...parts
          .filter(part => part.kind === "roofSlab" || part.kind === "mansardStep")
          .map(part => part.center[2] + part.size[2] / 2),
      );
    expect(roofTop(mansard)).toBeGreaterThan(roofTop(flat));
    expect(mansard.some(part => part.kind === "dormer")).toBe(true);
    expect(flat.some(part => part.kind === "roofSlab")).toBe(true);
  });

  it("normalizes unsafe parameters", () => {
    const normalized = normalizeParameters({
      baysX: 99,
      baysY: 0,
      floors: -3,
      groundFloorHeight: 100,
      floorHeight: 0.5,
      roofStyle: "pyramid" as never,
      balconies: "sometimes" as never,
      detail: "gaudy" as never,
    });
    expect(normalized).toEqual({
      baysX: 12,
      baysY: 1,
      floors: 1,
      groundFloorHeight: 8,
      floorHeight: 2.4,
      roofStyle: "mansard",
      balconies: "featured",
      detail: "alternating",
    });
  });

  it("keeps the footprint at bays × 3 m and parts above ground", () => {
    const parameters = { ...DEFAULT_PARAMETERS };
    const parts = generateBuildingParts(parameters, 1);
    const { width, depth } = { width: parameters.baysX * BAY, depth: parameters.baysY * BAY };
    for (const part of parts) {
      const halfW = part.size[0] / 2;
      const halfD = part.size[1] / 2;
      // 容差 1.3 m：阳台出挑（1.24 m）与檐口线脚是超出足迹的设计部分
      expect(part.center[0] + halfW).toBeLessThanOrEqual(width / 2 + 1.3);
      expect(part.center[0] - halfW).toBeGreaterThanOrEqual(-width / 2 - 1.3);
      expect(part.center[1] + halfD).toBeLessThanOrEqual(depth / 2 + 1.3);
      expect(part.center[1] - halfD).toBeGreaterThanOrEqual(-depth / 2 - 1.3);
      expect(part.center[2] - part.size[2] / 2).toBeGreaterThanOrEqual(-0.51);
    }
  });
});
