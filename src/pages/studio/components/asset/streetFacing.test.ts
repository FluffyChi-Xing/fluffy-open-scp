import { describe, expect, it } from "vitest";
import {
  DEFAULT_ROTATION_DEGREES,
  normalizeRotationDegrees,
  rotationFromStreetFacing,
  streetFacingFromRotation,
} from "./streetFacing";

describe("streetFacing", () => {
  it("默认朝向高亮 +Y（facing 0）对应构建 rotation 180", () => {
    expect(streetFacingFromRotation(DEFAULT_ROTATION_DEGREES)).toBe(0);
    expect(rotationFromStreetFacing(0)).toBe(180);
  });

  it("四向 roundtrip", () => {
    for (const facing of [0, 90, 180, 270] as const) {
      expect(streetFacingFromRotation(rotationFromStreetFacing(facing))).toBe(
        facing,
      );
    }
  });

  it("非法值回落默认 180", () => {
    expect(normalizeRotationDegrees(45)).toBe(180);
    expect(normalizeRotationDegrees(undefined)).toBe(180);
  });
});
