import { describe, expect, it } from "vitest";
import { RasterDocument } from "./document";
import { decalPreviewFamily, decalPreviewPixels } from "./decalPreview";
import { graffitiDisplayPixels } from "./graffitiPaint";
import { decalRoute } from "@/pages/packages/components/property-editor/decalRouting";

describe("2D Decal preview", () => {
  it("composites graffiti with paint strength and channel priority, including zero-A RGB masks", () => {
    const pixels = new Uint8ClampedArray([180, 0, 0, 0, 250, 0, 170, 0, 127, 0, 0, 0]);
    const output = graffitiDisplayPixels(pixels, ["#ff0000", "#00ff00", "#0000ff", "#ffffff"]);
    expect([...output]).toEqual([255, 0, 0, 180, 0, 0, 255, 170, 0, 0, 0, 0]);
    expect([...pixels]).toEqual([180, 0, 0, 0, 250, 0, 170, 0, 127, 0, 0, 0]);
  });
  it("uses the same material families as Property Editor", () => {
    expect(decalPreviewFamily("decal-sign")).toBe(
      decalRoute(0x73684efc).family,
    );
    expect(decalPreviewFamily("decal-sdf")).toBe(decalRoute(0x73684efc).family);
    expect(decalPreviewFamily("decal-graffiti")).toBe(
      decalRoute(0xe5390a98).family,
    );
    expect(decalPreviewFamily("decal-interior")).toBe(
      decalRoute(0x4491de3a).family,
    );
    expect(decalPreviewFamily("decal-rgba")).toBe(decalRoute(null).family);
  });
  it("preserves transparent RGB masks and row orientation without mutating the drawing", () => {
    const original = new Uint8ClampedArray([255, 0, 0, 0, 0, 255, 0, 128]);
    const doc = new RasterDocument(1, 2, original);
    expect([...decalPreviewPixels(doc)]).toEqual([
      0, 255, 0, 128, 255, 0, 0, 0,
    ]);
    expect(doc.pixels).toEqual(original);
  });
});
