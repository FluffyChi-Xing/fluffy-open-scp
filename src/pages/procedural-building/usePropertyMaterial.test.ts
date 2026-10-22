import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { effectScope, reactive } from "vue";
import { flushPromises } from "@vue/test-utils";
import { usePropertyMaterial } from "./usePropertyMaterial";

const mocks = vi.hoisted(() => ({
  readSession: vi.fn(),
  readMeshes: vi.fn(),
  analyze: vi.fn(),
  store: {} as any,
}));
vi.mock("@/stores/gamePackages", () => ({
  useGamePackagesStore: () => mocks.store,
}));
vi.mock("@/api/data-source", () => ({
  createDataSource: () => ({
    readLotEditorSession: mocks.readSession,
    readLotModelMeshes: mocks.readMeshes,
  }),
}));
vi.mock("@/lib/three-gltf", () => ({
  parseLotModelContainer: (value: unknown) => value,
}));
vi.mock(
  "@/lib/procedural-building/propertyMaterial",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    analyzeBuildingMaterial: mocks.analyze,
  }),
);

describe("automatic Property resolution", () => {
  let scope: ReturnType<typeof effectScope>;
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    mocks.store = reactive({
      opened: [
        { package: { packageId: 1, path: "graphics.package" } },
        { package: { packageId: 2, path: "ep1.package" } },
      ],
      activePackageId: 2,
    });
    mocks.readMeshes.mockResolvedValue({});
    mocks.analyze.mockResolvedValue({ payload: {}, facade: {}, roof: {} });
    scope = effectScope();
  });
  afterEach(() => {
    scope.stop();
    vi.useRealTimers();
  });
  const session = (name: string) => ({
    assetName: name,
    modelLods: [
      { packageId: 9, tgi: { typeId: 0x2f4e681b, group: 0, instance: 123 } },
    ],
  });

  it("debounces input, searches active package first, and follows the model's package", async () => {
    mocks.readSession
      .mockRejectedValueOnce(new Error("missing"))
      .mockResolvedValueOnce(session("Glass"));
    const state = scope.run(usePropertyMaterial)!;
    state.input.value = "00b1b104:61f0c000:1d261fdd";
    await vi.advanceTimersByTimeAsync(450);
    await flushPromises();
    expect(mocks.readSession.mock.calls.map((call) => call[0])).toEqual([2, 1]);
    expect(mocks.readMeshes).toHaveBeenCalledWith(
      9,
      expect.objectContaining({ instance: 123 }),
    );
    expect(state.resolved.value?.name).toBe("Glass");
    expect(state.resolved.value?.binding.packagePath).toBe("graphics.package");
    expect(state.loading.value).toBe(false);
  });

  it("does not publish an old request during the next input's debounce interval", async () => {
    let finish!: (value: unknown) => void;
    mocks.readSession.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const state = scope.run(usePropertyMaterial)!;
    state.input.value = "00b1b104:61f0c000:1d261fdd";
    await vi.advanceTimersByTimeAsync(450);
    state.input.value = "00b1b104:41e1c000:5a6ec67a";
    finish(session("Stale"));
    await flushPromises();
    expect(state.resolved.value).toBeNull();
    expect(mocks.readMeshes).not.toHaveBeenCalled();
    state.input.value = "";
    await vi.advanceTimersByTimeAsync(450);
    expect(state.loading.value).toBe(false);
    expect(state.errorKey.value).toBe("");
  });

  it("reports invalid identifiers, missing packages, and missing models", async () => {
    const state = scope.run(usePropertyMaterial)!;
    state.input.value = "bad";
    await state.resolve();
    expect(state.errorKey.value).toBe("propertyInvalidTgi");
    expect(mocks.readSession).not.toHaveBeenCalled();
    state.input.value = "00b1b104:61f0c000:1d261fdd";
    mocks.readSession.mockResolvedValue({ modelLods: [] });
    await state.resolve();
    expect(state.errorKey.value).toBe("propertyNoModel");
    mocks.store.opened = [];
    await state.resolve();
    expect(state.errorKey.value).toBe("propertyNoPackages");
    expect(state.resolved.value).toBeNull();
  });
});
