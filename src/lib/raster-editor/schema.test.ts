import { describe, expect, it } from "vitest";
import { inflateSync } from "node:zlib";
import { reactive } from "vue";
import { useRasterSession } from "@/components/raster/useRasterSession";
import { RasterDocument } from "./document";
import { base64ToRgba } from "./encoders";
import {
  channelPixels,
  parseRasterSchema,
  restoreOtherChannels,
  validateParameters,
} from "./schema";
import { encodeRgbaPng } from "./png";
import { rasterDraftAsset } from "@/pages/studio/components/mod-flow/assetFlow";

describe("Raster editing contract", () => {
  it("round-trips pixels under zero alpha, source identity and material settings", () => {
    const s = useRasterSession();
    s.changeProfile("decal-sdf");
    s.install(
      new RasterDocument(1, 1, new Uint8ClampedArray([43, 102, 211, 0])),
      { name: "original", tgi: { typeId: 0x2f4e681c, group: 9, instance: 10 } },
      "SDF",
    );
    s.decal.value.replaceInstance = "0x11";
    const snapshot = s.snapshot();
    const restored = useRasterSession(
      reactive(parseRasterSchema(JSON.stringify(snapshot))),
    );
    expect([...restored.doc.value!.pixels]).toEqual([43, 102, 211, 0]);
    expect(restored.snapshot()).toEqual(snapshot);
    restored.doc.value!.pixels[0] = 90;
    expect(s.doc.value!.pixels[0]).toBe(43);
  });
  it("rejects malformed image lengths, fractional sizes and unsupported versions", () => {
    const s = useRasterSession();
    s.create(1, 1);
    const value = s.snapshot();
    expect(() => parseRasterSchema({ ...value, schema_version: 2 })).toThrow(
      /版本/,
    );
    expect(() =>
      parseRasterSchema({ ...value, image: { ...value.image, width: 1.5 } }),
    ).toThrow(/整数/);
    expect(() =>
      parseRasterSchema({ ...value, image: { ...value.image, data: "AAAA" } }),
    ).toThrow(/像素长度/);
    expect(() => s.create(4097, 1)).toThrow();
  });
  it("keeps float32 parameter tables separate from pixel buffers", () => {
    const s = useRasterSession();
    s.changeProfile("slot0");
    s.parameters.value = {
      cols: 2,
      values: Array.from({ length: 32 }, (_, i) => i / 13),
    };
    const snapshot = s.snapshot();
    expect(snapshot.image).toBeNull();
    expect(snapshot.parameters!.values[1]).toBe(1 / 13);
    expect(() => validateParameters({ cols: 2, values: [0] })).toThrow();
    expect(() =>
      validateParameters({ cols: 1, values: Array(16).fill(1e40) }),
    ).toThrow();
    const asset = rasterDraftAsset(JSON.stringify(snapshot), 0);
    expect(JSON.parse(atob(asset.base64!))).toEqual(snapshot.parameters);
    expect(() => rasterDraftAsset(JSON.stringify(snapshot), 1)).toThrow(/槽位/);
  });
  it("edits one channel without destroying the other three", () => {
    const before = new Uint8ClampedArray([40, 50, 60, 70, 80, 90, 100, 0]);
    const after = new Uint8ClampedArray(8).fill(255);
    restoreOtherChannels(after, before, "g");
    expect([...after]).toEqual([40, 255, 60, 70, 80, 255, 100, 0]);
    expect([...channelPixels(before, "a")]).toEqual([
      70, 70, 70, 255, 0, 0, 0, 255,
    ]);
    expect([...before]).toEqual([40, 50, 60, 70, 80, 90, 100, 0]);
  });
  it("normal defaults are neutral and source replacement clears stale Lot targets", () => {
    const s = useRasterSession();
    s.changeProfile("slot2");
    s.create(1, 1);
    expect([...s.doc.value!.pixels]).toEqual([128, 128, 255, 255]);
    s.lot.value.target = { name: "old Lot" };
    s.create(2, 2);
    expect(s.lot.value.target).toBeNull();
    expect(s.sampling.value.generateMips).toBe(false);
  });
});

describe("lossless material PNG encoding", () => {
  it.each([
    [1, 1],
    [200, 100],
  ])(
    "preserves RGBA across deflate block boundaries (%i×%i)",
    (width, height) => {
      const pixels = Uint8ClampedArray.from(
        { length: width * height * 4 },
        (_, i) => (i % 4 === 3 ? 0 : i % 256),
      );
      const png = encodeRgbaPng(width, height, pixels);
      const view = new DataView(png.buffer);
      expect([...png.slice(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
      const length = view.getUint32(33);
      const scan = inflateSync(png.slice(41, 41 + length));
      for (let y = 0; y < height; y++) {
        const start = y * (width * 4 + 1);
        expect(scan[start]).toBe(0);
        expect([...scan.subarray(start + 1, start + width * 4 + 1)]).toEqual([
          ...pixels.subarray(y * width * 4, (y + 1) * width * 4),
        ]);
      }
      const s = useRasterSession();
      s.changeProfile("slot3");
      s.install(new RasterDocument(width, height, pixels), null, "shader");
      expect([
        ...base64ToRgba(
          rasterDraftAsset(JSON.stringify(s.snapshot()), 3).base64!,
        ),
      ]).toEqual([...png]);
    },
  );
});
