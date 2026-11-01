import { describe, expect, it } from "vitest";
import { applyAlphaCutout, hasTransparentPixels } from "./three-cutout";

describe("hasTransparentPixels", () => {
  it("detects any pixel below the clip threshold", () => {
    expect(hasTransparentPixels([255, 255, 255])).toBe(false);
    expect(hasTransparentPixels([255, 127, 255])).toBe(true);
    expect(hasTransparentPixels([])).toBe(false);
  });

  it("honors a custom threshold", () => {
    expect(hasTransparentPixels([200], 128)).toBe(false);
    expect(hasTransparentPixels([200], 255)).toBe(true);
  });
});

describe("applyAlphaCutout", () => {
  const material = () => ({ alphaTest: 0, needsUpdate: false });
  const texture = () => ({ image: { width: 4, height: 4 } });

  it("enables alphaTest 0.5 only when the scan reports transparency", () => {
    const cutoutMaterial = material();
    expect(applyAlphaCutout(cutoutMaterial, texture(), () => true)).toBe(true);
    expect(cutoutMaterial.alphaTest).toBe(0.5);
    expect(cutoutMaterial.needsUpdate).toBe(true);

    const opaqueMaterial = material();
    expect(applyAlphaCutout(opaqueMaterial, texture(), () => false)).toBe(false);
    expect(opaqueMaterial.alphaTest).toBe(0);
    expect(opaqueMaterial.needsUpdate).toBe(false);
  });

  it("skips scanning when there is no texture", () => {
    const mat = material();
    expect(applyAlphaCutout(mat, null, () => true)).toBe(false);
    expect(mat.alphaTest).toBe(0);
  });

  it("caches the scan decision per texture", () => {
    const matA = material();
    const matB = material();
    const tex = texture();
    let scans = 0;
    const scan = () => {
      scans += 1;
      return true;
    };
    applyAlphaCutout(matA, tex, scan);
    applyAlphaCutout(matB, tex, scan);
    expect(scans).toBe(1);
    expect(matB.alphaTest).toBe(0.5);
  });
});
