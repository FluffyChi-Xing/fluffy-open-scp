import { flushPromises, mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import { tauriApi } from "@/api";
import AssetComponentLibrary from "./AssetComponentLibrary.vue";

vi.mock("@/api", () => ({ tauriApi: { packages: { listModelCatalog: vi.fn(), listPropBinCatalog: vi.fn() } } }));

it("requests only the supported prop library and never displays misc or arbitrary referenced models", async () => {
  vi.mocked(tauriApi.packages.listPropBinCatalog).mockResolvedValue([
    ...["trees", "vehicles", "waste", "signs", "misc"].map((category, index) => ({
      resourceId: index, name: `item-${category}`, category,
      model: { typeId: 1, group: 0, instance: index }, modelPackageId: 1,
      modelPackagePath: "Game.package", source: "explicit",
    })),
  ] as Awaited<ReturnType<typeof tauriApi.packages.listPropBinCatalog>>);
  const wrapper = mount(AssetComponentLibrary, {
    props: { editEnabled: true, propTreeIds: new Set<number>(), templateOf: () => undefined,
      referencedModels: [{ packageId: 1, group: 0, instance: 999, resourceId: 999 }] },
    global: { plugins: [i18n], stubs: { AssetDecalLibrary: true, SpawnerLibraryPanel: true } },
  });
  try {
    await flushPromises();
    expect(tauriApi.packages.listModelCatalog).not.toHaveBeenCalled();
    expect(wrapper.findAll(".library-group")).toHaveLength(4);
    expect(wrapper.findAll(".library-entry").map((entry) => entry.text())).toEqual([
      "item-trees", "item-vehicles", "item-waste", "item-signs",
    ]);
    await wrapper.get(".library-entry").trigger("pointerdown");
    expect(wrapper.emitted("place")?.[0]?.[0]).toMatchObject({ packagePath: "Game.package" });
  } finally { wrapper.unmount(); }
});
