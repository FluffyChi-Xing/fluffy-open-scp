import { describe, expect, it } from "vitest";
import { RASTER_PROFILES, restoreOtherChannels } from "./schema";
import {
  PROFILE_BEHAVIOR,
  dataChannelPixels,
  layerPaint,
  pickedLayer,
} from "./profileBehavior";

describe("Raster profile contracts", () => {
  it.each(RASTER_PROFILES.map((profile) => profile.id))(
    "defines explicit behavior for %s",
    (profile) => {
      expect(PROFILE_BEHAVIOR[profile]).toBeDefined();
      expect(typeof PROFILE_BEHAVIOR[profile].dataChannels).toBe("boolean");
    },
  );
  it("keeps zero-alpha SDF and normal data visible without altering the source", () => {
    const pixels = new Uint8ClampedArray([23, 160, 91, 0]);
    expect([...dataChannelPixels(pixels)]).toEqual([23, 160, 91, 255]);
    expect([...pixels]).toEqual([23, 160, 91, 0]);
    expect(PROFILE_BEHAVIOR["decal-sdf"].defaultChannel).toBe("r");
    expect(PROFILE_BEHAVIOR["decal-rgba"].dataChannels).toBe(false);
  });
  it("roundtrips every Lot and Decal paint layer independently", () => {
    for (const profile of [
      "lot-mask",
      "decal-sign",
      "decal-graffiti",
    ] as const) {
      for (let layer = 0; layer < 4; layer++)
        expect(pickedLayer(profile, layerPaint(profile, layer))).toBe(layer);
    }
    expect(layerPaint("lot-mask", 0)).toEqual([0, 0, 0, 255]);
    expect(layerPaint("decal-sign", 0)).toEqual([255, 0, 0, 0]);
    expect(layerPaint("decal-graffiti", 1, 150)).toEqual([0, 150, 0, 0]);
  });
  it("does not present unused four-color controls for interior or RGBA", () => {
    expect(PROFILE_BEHAVIOR["decal-interior"].palette).toBe(false);
    expect(PROFILE_BEHAVIOR["decal-rgba"].palette).toBe(false);
    expect(PROFILE_BEHAVIOR["decal-graffiti"].palette).toBe(true);
    expect(PROFILE_BEHAVIOR["decal-sdf"].palette).toBe(true);
  });
  it("single-channel distance editing preserves the other fields", () => {
    const before = new Uint8ClampedArray([1, 2, 3, 0]);
    const after = new Uint8ClampedArray([170, 170, 170, 255]);
    restoreOtherChannels(after, before, "r");
    expect([...after]).toEqual([170, 2, 3, 0]);
  });
});
