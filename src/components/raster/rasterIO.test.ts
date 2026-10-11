import { beforeEach, describe, expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import { loadLotCopy, saveRasterCopy } from "./rasterIO";
import { useRasterSession } from "./useRasterSession";
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
