import { beforeEach, describe, expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import type { LotEditorSession } from "@/api/tauri";
import { loadLotPreviewSession } from "./lotPreviewSession";
vi.mock("@/api", () => ({
  tauriApi: {
    raster: { readLotPreviewAtlases: vi.fn(), listPropertyDocuments: vi.fn() },
    packages: { readLotEditorSession: vi.fn() },
  },
}));
const tgi = (instance: number) => ({ typeId: 1, group: 2, instance });
const session = (value: Partial<LotEditorSession>) => value as LotEditorSession;
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(tauriApi.raster.readLotPreviewAtlases).mockResolvedValue(null);
});
describe("Lot preview atlas discovery", () => {
  it("previews a new mask from Graphics alone without requiring a Lot Property", async () => {
    const atlas = { lotSurfacePng: "game", lotNormalAtlasPng: "normal" };
    vi.mocked(tauriApi.raster.readLotPreviewAtlases).mockResolvedValue(atlas);
    expect(await loadLotPreviewSession(null, [1], () => false)).toEqual({
      bound: null,
      atlas,
    });
    expect(tauriApi.raster.listPropertyDocuments).not.toHaveBeenCalled();
  });
  it("skips empty and failing candidates and preserves the bound Lot geometry", async () => {
    const bound = session({ lotSize: [24, 48], lotSurfacePng: null });
    vi.mocked(tauriApi.raster.listPropertyDocuments).mockResolvedValue(
      [1, 2, 3, 4].map((instance) => ({ tgi: tgi(instance) })) as never,
    );
    vi.mocked(tauriApi.packages.readLotEditorSession)
      .mockResolvedValueOnce(bound)
      .mockResolvedValueOnce(session({ lotSurfacePng: null }))
      .mockRejectedValueOnce(new Error("broken candidate"))
      .mockResolvedValueOnce(
        session({ lotSize: [99, 99], lotSurfacePng: "valid" }),
      );
    const result = await loadLotPreviewSession(
      { name: "edited", packageId: 1, tgi: tgi(1) },
      [1],
      () => false,
    );
    expect(result.bound).toBe(bound);
    expect(result.bound?.lotSize).toEqual([24, 48]);
    expect(result.atlas?.lotSurfacePng).toBe("valid");
    expect(tauriApi.packages.readLotEditorSession).toHaveBeenCalledTimes(4);
  });
  it("stops discovery when the editor switches documents", async () => {
    expect(await loadLotPreviewSession(null, [1], () => true)).toEqual({
      bound: null,
      atlas: null,
    });
    expect(tauriApi.raster.readLotPreviewAtlases).not.toHaveBeenCalled();
    expect(tauriApi.raster.listPropertyDocuments).not.toHaveBeenCalled();
  });
});
