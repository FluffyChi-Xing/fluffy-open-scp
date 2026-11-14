import { describe, expect, it } from "vitest";
import {
  DEFAULT_STREET_FACING,
  normalizeStreetFacing,
  streetFacingFromLegacyRotation,
} from "./streetFacing";

describe("streetFacing", () => {
  it("默认临街边是 +Y（facing 0）", () => {
    expect(DEFAULT_STREET_FACING).toBe(0);
    expect(normalizeStreetFacing(undefined)).toBe(0);
  });

  it("只接受四向直角", () => {
    for (const facing of [0, 90, 180, 270] as const) {
      expect(normalizeStreetFacing(facing)).toBe(facing);
    }
    expect(normalizeStreetFacing(45)).toBe(0);
  });

  it("旧 rotationDegrees 口径可迁移到 facing", () => {
    expect(streetFacingFromLegacyRotation(180)).toBe(0);
    expect(streetFacingFromLegacyRotation(270)).toBe(90);
    expect(streetFacingFromLegacyRotation(0)).toBe(180);
    expect(streetFacingFromLegacyRotation(90)).toBe(270);
  });
});
