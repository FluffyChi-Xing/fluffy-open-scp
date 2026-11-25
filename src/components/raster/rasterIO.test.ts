import { beforeEach, describe, expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import { loadLotCopy, saveRasterCopy } from "./rasterIO";
import { useRasterSession } from "./useRasterSession";
import { rgbaToBase64 } from "@/lib/raster-editor/encoders";
vi.mock("@/api", () => ({
  tauriApi: {
    packages: { saveFile: vi.fn(), readLotEditorSession: vi.fn() },
    raster: {
      saveOverlay: vi.fn(),
      createLotOverlay: vi.fn(),
      registerDecalEntry: vi.fn(),
      readRgba: vi.fn(),
    },
  },
}));
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(tauriApi.packages.saveFile).mockResolvedValue("copy.package");
});
describe("native Raster copy adapters", () => {
  it("exports graffiti as raw paint channels and enforces the non-emissive dictionary", async () => {
    const session = useRasterSession();
    session.changeProfile("decal-graffiti");
    session.create(2, 1);
    const pixels = new Uint8ClampedArray([180, 0, 0, 0, 0, 0, 0, 220]);
    session.doc.value!.pixels.set(pixels);
    const atlas = { packageId: 42, tgi: { typeId: 0xb1b104, group: 0xfb661651, instance: 0xeefd390c } };
    session.decal.value.atlas = { name: "graffiti", ...atlas };
    await expect(saveRasterCopy(session.snapshot(), "decal")).resolves.toBe("copy.package");
    expect(tauriApi.raster.registerDecalEntry).toHaveBeenCalledWith(expect.objectContaining({
      atlas,
      rgbaBase64: rgbaToBase64(pixels),
      expectedMaterialInstance: 0xe5390a98,
      replaceInstance: undefined,
    }));
    session.decal.value.layers = Array.from({ length: 4 }, (_, index) => ({
      visible: index !== 0, locked: index === 0,
    }));
    const draft = session.snapshot();
    await saveRasterCopy(draft, "decal");
    expect(tauriApi.raster.registerDecalEntry).toHaveBeenLastCalledWith(expect.objectContaining({
      rgbaBase64: rgbaToBase64(new Uint8ClampedArray([0, 0, 0, 0, 0, 0, 0, 220])),
    }));
    expect(draft.image?.data).toBe(rgbaToBase64(pixels));
  });
  it("does not save a marquee draft as a package without its animation settings", async () => {
    const session = useRasterSession();
    session.changeProfile("decal-sign");
    session.create(1, 1);
    session.decal.value.sign = { effect: "marquee", axis: "horizontal", speed: 1 };
    await expect(saveRasterCopy(session.snapshot(), "decal")).rejects.toThrow(/JSON/);
    expect(tauriApi.packages.saveFile).not.toHaveBeenCalled();
    expect(tauriApi.raster.registerDecalEntry).not.toHaveBeenCalled();
  });
  it("copies Property Editor colors and pattern indices instead of debug defaults", async () => {
    const colors = [
      [110, 120, 130, 3],
      [80, 90, 100, 1],
      [150, 160, 170, 8],
      [190, 180, 170, 3],
    ];
    vi.mocked(tauriApi.raster.readRgba).mockResolvedValue({
      width: 1,
      height: 1,
      decodable: true,
      rgbaBase64: "AAAA/w==",
    } as never);
    vi.mocked(tauriApi.packages.readLotEditorSession).mockResolvedValue({
      lotColors: colors,
      lotColorsAuthored: [true, true, true, true],
    } as never);
    const summary = {
      packageId: 2,
      tgi: { typeId: 1, group: 2, instance: 3 },
      lotMaskResolved: { typeId: 0x2f4e681c, group: 4, instance: 5 },
      lotMaskPackageId: 1,
    } as Parameters<typeof loadLotCopy>[0];
    const copy = await loadLotCopy(summary, [1, 2]);
    expect(copy.material.colors).toEqual(colors);
    expect(tauriApi.packages.readLotEditorSession).toHaveBeenCalledWith(
      2,
      summary.tgi,
    );
    vi.mocked(tauriApi.packages.readLotEditorSession).mockResolvedValue({
      lotColors: colors,
      lotColorsAuthored: [false, false, false, false],
    } as never);
    await expect(loadLotCopy(summary, [1, 2])).rejects.toThrow(/材质颜色/);
  });
  it("never writes Raster bytes under an RW4 source identity", async () => {
    const s = useRasterSession();
    s.create(1, 1);
    s.source.value = {
      name: "texture",
      tgi: { typeId: 0x2f4e681b, group: 42, instance: 77 },
    };
    await expect(saveRasterCopy(s.snapshot(), "raster")).rejects.toThrow(/RW4/);
    expect(tauriApi.packages.saveFile).not.toHaveBeenCalled();
    expect(tauriApi.raster.saveOverlay).not.toHaveBeenCalled();
  });
  it("keeps the source TGI when exporting an edited copy", async () => {
    const s = useRasterSession();
    s.create(1, 1);
    s.source.value = {
      name: "source",
      tgi: { typeId: 0x2f4e681c, group: 42, instance: 77 },
    };
    await saveRasterCopy(s.snapshot(), "raster");
    expect(tauriApi.raster.saveOverlay).toHaveBeenCalledWith(
      expect.objectContaining({
        tgi: s.source.value.tgi,
        outputPath: "copy.package",
        generateMips: false,
      }),
    );
  });
  it("canceling the save picker does not write anything", async () => {
    vi.mocked(tauriApi.packages.saveFile).mockResolvedValue(null);
    const s = useRasterSession();
    s.create(1, 1);
    expect(await saveRasterCopy(s.snapshot(), "raster")).toBeNull();
    expect(tauriApi.raster.saveOverlay).not.toHaveBeenCalled();
  });
  it("creates Lot colors and pattern indices without converting tile alpha to opacity", async () => {
    const s = useRasterSession();
    s.create(1, 1);
    s.lot.value.colors[0] = [128, 64, 32, 15];
    await saveRasterCopy(s.snapshot(), "lot");
    expect(tauriApi.raster.createLotOverlay).toHaveBeenCalledWith(
      expect.objectContaining({ colors: s.lot.value.colors }),
    );
    s.sampling.value.metersPerPixel = 1;
    await expect(saveRasterCopy(s.snapshot(), "lot")).rejects.toThrow(/0.75/);
  });
  it("preserves an asymmetric Lot's columns and game row order through import and export", async () => {
    const gameRows = new Uint8ClampedArray([
      255, 0, 0, 17, 255, 0, 0, 23,
      255, 0, 0, 31, 0, 0, 0, 0,
      255, 0, 0, 47, 0, 0, 0, 0,
    ]);
    const rgbaBase64 = rgbaToBase64(gameRows);
    vi.mocked(tauriApi.raster.readRgba).mockResolvedValue({
      width: 2, height: 3, decodable: true, rgbaBase64,
    } as never);
    vi.mocked(tauriApi.packages.readLotEditorSession).mockResolvedValue({
      lotColors: [[100, 100, 100, 0], [100, 100, 100, 0], [100, 100, 100, 0], [100, 100, 100, 0]],
      lotColorsAuthored: [true, true, true, true],
    } as never);
    const copy = await loadLotCopy({
      packageId: 2,
      tgi: { typeId: 0x00b1b104, group: 2, instance: 3 },
      lotMaskResolved: { typeId: 0x2f4e681c, group: 4, instance: 5 },
    } as Parameters<typeof loadLotCopy>[0], []);
    expect(copy.image.pixels).toEqual(new Uint8ClampedArray([
      ...gameRows.slice(16), ...gameRows.slice(8, 16), ...gameRows.slice(0, 8),
    ]));
    const session = useRasterSession();
    session.changeProfile("lot-mask");
    session.create(2, 3);
    session.doc.value!.pixels.set(copy.image.pixels);
    await saveRasterCopy(session.snapshot(), "lot");
    expect(tauriApi.raster.createLotOverlay).toHaveBeenCalledWith(expect.objectContaining({ rgbaBase64 }));
  });
  it("refuses resizing a Lot override", async () => {
    const s = useRasterSession();
    s.create(1, 1);
    s.source.value = {
      name: "mask",
      packageId: 1,
      tgi: { typeId: 0x2f4e681c, group: 0, instance: 1 },
    };
    s.lot.value.target = {
      name: "Lot",
      packageId: 2,
      tgi: { typeId: 0x00b1b104, group: 0, instance: 2 },
    };
    vi.mocked(tauriApi.raster.readRgba).mockResolvedValue({
      width: 64,
      height: 64,
      decodable: true,
      pixelFormat: 21,
      mipCount: 1,
    });
    await expect(saveRasterCopy(s.snapshot(), "raster")).rejects.toThrow(
      /尺寸/,
    );
    expect(tauriApi.packages.saveFile).not.toHaveBeenCalled();
  });
  it("validates complete hex IDs and preserves replacement identity with linear half colors", async () => {
    const s = useRasterSession();
    s.changeProfile("decal-sign");
    s.create(1, 1);
    s.decal.value.atlas = {
      name: "atlas",
      packageId: 1,
      tgi: { typeId: 0x00b1b104, group: 3, instance: 4 },
    };
    s.decal.value.replaceInstance = "0x12XYZ";
    await expect(saveRasterCopy(s.snapshot(), "decal")).rejects.toThrow(
      /十六进制/,
    );
    s.decal.value.replaceInstance = "0xFFFFFFFF";
    await saveRasterCopy(s.snapshot(), "decal");
    expect(tauriApi.raster.registerDecalEntry).toHaveBeenCalledWith(
      expect.objectContaining({
        replaceInstance: 0xffffffff,
        entryId: { typeId: 0, group: 0, instance: 0xffffffff },
        colors: Array(4).fill([0.5, 0.5, 0.5, 0]),
      }),
    );
  });
});
