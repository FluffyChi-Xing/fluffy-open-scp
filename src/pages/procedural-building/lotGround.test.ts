import { describe, expect, it, vi } from "vitest";
import { effectScope } from "vue";
import {
  loadLotGround,
  lotKey,
  lotLayerPixels,
  useLotGround,
  type LotSelection,
} from "./lotGround";
import { tauriApi } from "@/api";
import type { LotEditorSession } from "@/api/tauri";
import {
  rgbaToBase64,
  rgbaBase64ToPngDataUrl,
} from "@/lib/raster-editor/encoders";

vi.mock("@/api", () => ({
  tauriApi: {
    packages: { readLotEditorSession: vi.fn() },
    raster: { readSource: vi.fn() },
  },
}));
vi.mock("@/lib/raster-editor/encoders", async (original) => ({
  ...(await original<typeof import("@/lib/raster-editor/encoders")>()),
  rgbaBase64ToPngDataUrl: vi.fn(() => "data:image/png;base64,UE5H"),
}));
const selection: LotSelection = {
  packageId: 2,
  tgi: { typeId: 3, group: 4, instance: 5 },
  label: "Lot A",
  lotPackageId: 7,
  lotTgi: { typeId: 1, group: 2, instance: 3 },
  lotSize: [136, 80],
};
const colors: LotEditorSession["lotColors"] = [
  [200, 190, 175, 0],
  [220, 220, 220, 1],
  [160, 150, 125, 2],
  [90, 100, 90, 3],
];
const raw = (bytes: number[]) => rgbaToBase64(new Uint8ClampedArray(bytes));
const session = {
  lotSize: [136, 80],
  lotMaskPng: "UE5H",
  lotAlbedoPng: "OLD",
  lotMaskRawRgba: raw([0, 0, 255, 0, 255, 0, 0, 0]),
  lotSurfacePng: "SURFACE",
  lotNormalAtlasPng: "NORMAL",
  lotColors: colors,
  lotColorsAuthored: [true, true, true, true],
  lotBorderColors: [[10, 20, 30]],
  lotBorderWidths: [0.1, 0, 0, 0],
  lotBorderPatternIndices: [1, 2, 3, 4],
  lotBaseTile: 8,
  lotTilePeriod: [8, 4],
  lotPlacement: [1, 0, 0, 0, 1, 0, 0, 0, 1, 3, 4, 0],
  lotOverlayBoxOffset: [7, 8],
  lotModelBBoxCenter: [9, 10],
  units: [{ kind: "light" }],
  decalTextures: [{}],
  pathPairs: [1],
} as LotEditorSession;

describe("procedural Lot ground contract", () => {
  it("uses authored colors and A > B > G > R priority, without alpha premultiplication", () => {
    for (const [pixel, color] of [
      [[255, 0, 0, 0], colors[0]],
      [[255, 255, 0, 0], colors[1]],
      [[255, 255, 255, 0], colors[2]],
      [[255, 255, 255, 255], colors[3]],
    ] as const) {
      const result = lotLayerPixels(
        { width: 1, height: 1, rgbaBase64: raw([...pixel]) },
        colors,
        1,
      );
      expect([...result.albedo]).toEqual([...color.slice(0, 3), 255]);
    }
  });
  it("keeps cross-package Lot identity, original size, world-oriented weights and refined fields", async () => {
    vi.mocked(tauriApi.packages.readLotEditorSession).mockResolvedValue(
      session,
    );
    vi.mocked(tauriApi.raster.readSource).mockResolvedValue({
      width: 1,
      height: 2,
      rgbaBase64: raw([255, 0, 0, 0, 0, 0, 255, 0]),
      parameters: null,
    });
    const result = await loadLotGround(selection);
    expect(tauriApi.packages.readLotEditorSession).toHaveBeenCalledWith(
      7,
      selection.lotTgi,
    );
    expect(tauriApi.raster.readSource).toHaveBeenCalledWith(2, selection.tgi);
    expect(result.session).toMatchObject({
      ...session,
      lotAlbedoPng: "OLD",
      units: [],
      decalTextures: [],
      pathPairs: [],
    });
    expect(result.session.lotMaskPng).not.toContain("data:");
    const call = vi.mocked(rgbaBase64ToPngDataUrl).mock.calls.at(-1)!;
    const worldFlat = lotLayerPixels(
      { width: 1, height: 2, rgbaBase64: session.lotMaskRawRgba },
      colors,
      1024,
    );
    expect(call[0]).toBe(rgbaToBase64(worldFlat.albedo));
    expect(lotKey(selection)).not.toBe(
      lotKey({ ...selection, lotTgi: { ...selection.lotTgi, group: 99 } }),
    );
  });
  it("discards superseded, cleared and disposed requests", async () => {
    const pending: ((v: Awaited<ReturnType<typeof loadLotGround>>) => void)[] =
      [];
    const scope = effectScope();
    const state = scope.run(() =>
      useLotGround(() => new Promise((resolve) => pending.push(resolve))),
    )!;
    const value = { session, dataUrl: "png", selection };
    const first = state.select(selection),
      second = state.select({ ...selection, label: "B" });
    pending[1]({ ...value, selection: { ...selection, label: "B" } });
    await second;
    pending[0](value);
    expect(await first).toBe(false);
    expect(state.selected.value?.selection.label).toBe("B");
    const third = state.select(selection);
    state.clear();
    pending[2](value);
    await third;
    expect(state.selected.value).toBeNull();
    const fourth = state.select(selection);
    scope.stop();
    pending[3](value);
    await fourth;
    expect(state.selected.value).toBeNull();
    expect(state.loading.value).toBe(false);
  });
});
