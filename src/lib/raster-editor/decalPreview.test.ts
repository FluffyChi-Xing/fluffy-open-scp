import { describe, expect, it } from "vitest";
import { RasterDocument } from "./document";
import { decalPreviewFamily, decalPreviewPixels } from "./decalPreview";
import { decalRoute } from "@/pages/packages/components/property-editor/decalRouting";

describe("2D Decal preview", () => {
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
