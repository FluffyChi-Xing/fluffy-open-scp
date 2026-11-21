import { defineComponent, provide } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, expect, it, vi } from "vitest";
import { EDITOR_PACKAGES, useEditorPackages } from "@/composables/useEditorPackages";
import { useWorkbenchAsset } from "./useWorkbenchAsset";

const mock = vi.hoisted(() => ({ command: vi.fn(), readSession: vi.fn() }));
vi.mock("@/api", () => ({
  isTauri: () => false,
  tauriApi: { settings: { get: async () => ({ gameDataPath: "Game" }) } },
}));
vi.mock("@/api/tauri", () => ({ command: mock.command }));
vi.mock("@/api/data-source", () => ({
  createDataSource: () => ({ readLotEditorSession: mock.readSession }),
}));

afterEach(() => vi.resetAllMocks());

it("opens native scenes with the package scope owned by the same component", async () => {
  mock.command.mockResolvedValue({ packageId: 303, path: "D:/Game/SimCity_Game.package" });
  mock.readSession.mockResolvedValue({ units: [], modelLods: [], lotSize: [32, 32] });
  let asset!: ReturnType<typeof useWorkbenchAsset>;
  const wrapper = mount(defineComponent({
    setup() {
      const packages = useEditorPackages("assets");
      provide(EDITOR_PACKAGES, packages);
      asset = useWorkbenchAsset("project", false, packages);
      return () => null;
    },
  }));
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
