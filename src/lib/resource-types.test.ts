import { describe, expect, it } from "vitest";
import {
  PROPERTY_TYPE_ID,
  PROPERTY_INSTANCE_TYPES,
  isDecalAtlasGroup,
  propertyInstanceKey,
} from "./resource-types";

describe("property InstanceType discriminator", () => {
  // 判据 = GroupContainer & 0xFFFF（原 SCP DataBaseIndex.cs）。
  // group 高 16 位是同子类型的分卷编号，必须被忽略。
  it("ignores the high 16 bits of the group container", () => {
    // 实测：SimCity_Game.package 里 lot 0xEE27D643 的 group
    expect(propertyInstanceKey(PROPERTY_TYPE_ID, 0x42e1c000)).toBe(
      PROPERTY_INSTANCE_TYPES[0xc000],
    );
    // 同一 lot 的父级，另一个分卷编号
    expect(propertyInstanceKey(PROPERTY_TYPE_ID, 0x40e1c000)).toBe(
      PROPERTY_INSTANCE_TYPES[0xc000],
    );
    // 任意分卷号都不该影响判别
    expect(propertyInstanceKey(PROPERTY_TYPE_ID, 0xdeadc000)).toBe(
      PROPERTY_INSTANCE_TYPES[0xc000],
    );
  });

  it("maps the documented PropertyFileTypeIds values", () => {
    const cases: [number, number][] = [
      [0xc600, 0xc600],
      [0xc400, 0xc400],
      [0x8b7e, 0x8b7e],
      [0xc900, 0xc900],
      [0x8a01, 0x8a01],
      [0xe000, 0xe000],
      [0x2043, 0x2043],
    ];
    for (const [instanceType, expected] of cases) {
      expect(propertyInstanceKey(PROPERTY_TYPE_ID, instanceType)).toBe(
        PROPERTY_INSTANCE_TYPES[expected],
      );
    }
  });

  it("treats the three DecalAtlas volumes as decal dictionaries", () => {
    for (const instanceType of [0xb185, 0x1651, 0x1652]) {
      expect(isDecalAtlasGroup(instanceType)).toBe(true);
      expect(propertyInstanceKey(PROPERTY_TYPE_ID, instanceType)).toContain(
        "decalAtlas",
      );
    }
    expect(isDecalAtlasGroup(0xc000)).toBe(false);
  });

  it("returns null for non-property resources and unknown instance types", () => {
    // RW4 / Raster 没有 InstanceType 概念
    expect(propertyInstanceKey(0x2f4e681b, 0x42e1c000)).toBeNull();
    expect(propertyInstanceKey(0x2f4e681c, 0x42e1c000)).toBeNull();
    // Property 但 InstanceType 未知
    expect(propertyInstanceKey(PROPERTY_TYPE_ID, 0x1234)).toBeNull();
  });
});
