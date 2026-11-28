import { flushPromises } from "@vue/test-utils";
import { effectScope, nextTick, reactive, ref } from "vue";
import { expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import type { EditorPackageScope } from "@/composables/useEditorPackages";
import { useLotAtlas } from "./lotAtlas";

vi.mock("@/api", () => ({
  isTauri: () => true,
  tauriApi: { raster: { readLotPreviewAtlases: vi.fn() } },
}));

it("waits for game packages and does not cache an empty atlas across package changes", async () => {
  const read = vi.mocked(tauriApi.raster.readLotPreviewAtlases);
  read.mockResolvedValueOnce(null).mockResolvedValueOnce({
    lotSurfacePng: "surface", lotNormalAtlasPng: null,
  });
  const packages = reactive<EditorPackageScope>({
    loading: true, opened: [], directory: "", missing: [], failed: false,
    ready: Promise.resolve(), load: vi.fn(), openFile: vi.fn(),
  });
  const scope = effectScope();
  try {
    const first = scope.run(() => useLotAtlas(packages, ref(true)))!;
    const second = scope.run(() => useLotAtlas(packages, ref(true)))!;
    expect(read).not.toHaveBeenCalled();
    packages.loading = false;
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(1);
    expect(first.value).toBeNull();
    packages.loading = true;
    await nextTick();
    packages.opened = [{ package: { packageId: 10, path: "SimCity_Graphics.package" } }];
    packages.loading = false;
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(2);
    expect(first.value).toBe("surface");
    expect(second.value).toBe("surface");
  } finally {
    scope.stop();
  }
});
