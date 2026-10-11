import { beforeEach, describe, expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import { saveRasterCopy } from "./rasterIO";
import { useRasterSession } from "./useRasterSession";
vi.mock("@/api", () => ({
  tauriApi: {
    packages: { saveFile: vi.fn() },
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
