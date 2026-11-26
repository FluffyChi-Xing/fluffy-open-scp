import { defineComponent, provide } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, expect, it, onTestFinished, vi } from "vitest";
import { EDITOR_PACKAGES, useEditorPackages } from "@/composables/useEditorPackages";
import { useWorkbenchAsset } from "./useWorkbenchAsset";

const mock = vi.hoisted(() => ({
  command: vi.fn(), readSession: vi.fn(), readText: vi.fn(), resolveProps: vi.fn(),
}));
vi.mock("@/api", () => ({
  isTauri: () => false,
  tauriApi: {
    settings: { get: async () => ({ gameDataPath: "Game" }) },
    workspace: {
      codeReadText: mock.readText,
      codeTree: async () => ({ rootPath: "D:/project" }),
    },
    packages: { readLotEditorSession: mock.readSession },
  },
}));
vi.mock("@/api/tauri", () => ({ command: mock.command }));
vi.mock("@/api/data-source", () => ({
  createDataSource: () => ({
    readLotEditorSession: mock.readSession,
    resolvePropModels: mock.resolveProps,
  }),
}));

afterEach(() => vi.resetAllMocks());

function mountAsset() {
  let asset!: ReturnType<typeof useWorkbenchAsset>;
  const wrapper = mount(defineComponent({
    setup() {
      const packages = useEditorPackages("assets");
      provide(EDITOR_PACKAGES, packages);
      asset = useWorkbenchAsset("project", false, packages);
      return () => null;
    },
  }));
  return { asset, wrapper };
}

it("opens native scenes with the package scope owned by the same component", async () => {
  mock.command.mockResolvedValue({ packageId: 303, path: "D:/Game/SimCity_Game.package" });
  mock.readSession.mockResolvedValue({ units: [], modelLods: [], lotSize: [32, 32] });
  const { asset, wrapper } = mountAsset();
  try {
    await flushPromises();
    const tgi = { typeId: 0x00b1b104, group: 0x40e1c000, instance: 42 };
    await asset.openNative("D:/Game/SimCity_Game.package", tgi);
    await flushPromises();
    expect(mock.command).toHaveBeenCalledWith("acquire_editor_file", { path: "D:/Game/SimCity_Game.package" });
    expect(mock.readSession).toHaveBeenCalledWith(303, tgi);
    expect(asset.ui.value?.loadError).toBe("");
    expect(asset.schemaDocument.value).not.toBeNull();
  } finally {
    wrapper.unmount();
  }
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (cause: Error) => void;
  const promise = new Promise<T>((onResolve, onReject) => {
    resolve = onResolve;
    reject = onReject;
  });
  return { promise, resolve, reject };
}

it.each(["donor loaded", "props loaded first", "donor missing", "scene closed"] as const)(
  "publishes the restored model, ground and units together when %s",
  async (outcome) => {
    const assetPath = "assets/models/building.glb";
    const donor = deferred<{ lotAlbedoPng: string }>();
    const props = deferred<{ resolutions: [] }>();
    mock.readText.mockResolvedValue({
      content: JSON.stringify({
        $schema: "openscp.lot-asset/1",
        editor: {
          workbench: {
            source: "glb",
            assetPath,
            lotSize: [32, 48],
            lotStyle: {
              colors: [], colorsAuthored: [], borderColors: [], borderWidths: [],
              borderPatternIndices: [], baseTile: 8,
              sourceLot: { packageId: 12, tgi: { typeId: 1, group: 2, instance: 3 } },
            },
          },
        },
        units: [
          { kind: "light", index: 1 },
          { kind: "prop", index: 2, resourceId: 3 },
          { kind: "pathPoint", index: 3, pointIndex: 0, point: [1, 2, 0] },
        ],
      }),
    });
    mock.command.mockImplementation(async (_command, args) => {
      const path = args.request?.payload?.asset;
      if (path === assetPath) return { base64: btoa('{"asset":{"version":"2.0"},"scenes":[{}]}') };
      if (path === "assets/lot/mask.png") return { base64: "mask" };
      if (path === "assets/lot/albedo.png") return { base64: "saved" };
      throw new Error("missing asset");
    });
    mock.readSession.mockReturnValue(donor.promise);
    mock.resolveProps.mockReturnValue(props.promise);
    const { asset, wrapper } = mountAsset();
    onTestFinished(() => wrapper.unmount());
    const restore = asset.restoreFromProject();
    await vi.waitFor(() => {
      expect(mock.readSession).toHaveBeenCalledOnce();
      expect(mock.resolveProps).toHaveBeenCalledOnce();
    });
    expect(asset.ui.value).toBeNull();
    expect(asset.restoring.value).toBe(true);
    if (outcome === "scene closed") asset.closeAsset();
    if (outcome === "props loaded first") {
      props.resolve({ resolutions: [] });
      await flushPromises();
      expect(asset.ui.value).toBeNull();
    }
    if (outcome === "donor missing") donor.reject(new Error("stale donor package"));
    else donor.resolve({ lotAlbedoPng: "donor" });
    if (outcome !== "props loaded first") {
      await flushPromises();
      expect(asset.ui.value).toBeNull();
      props.resolve({ resolutions: [] });
    }
    expect(await restore).toBe(true);
    expect(asset.restoring.value).toBe(false);
    if (outcome === "scene closed") {
      expect(asset.ui.value).toBeNull();
      return;
    }
    const scene = asset.ui.value!;
    expect(scene.modelPayload?.glbs).toHaveLength(1);
    expect(scene.lotSize).toEqual([32, 48]);
    expect(scene.lotAlbedoPng).toBe(`data:image/png;base64,${outcome === "donor missing" ? "saved" : "donor"}`);
    expect(scene.grouping.lights).toHaveLength(1);
    expect(scene.grouping.props).toHaveLength(1);
    expect(scene.grouping.pathPoints).toHaveLength(1);
    expect(asset.savedDirty.value).toBe(false);
  },
);
