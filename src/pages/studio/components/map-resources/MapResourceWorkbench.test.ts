import { flushPromises, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { i18n } from "@/locales";
import type { Region3DData } from "@/lib/region-map";
import MapResourceWorkbench from "./MapResourceWorkbench.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/api", () => ({
  tauriApi: { workspace: { codeReadText: vi.fn().mockResolvedValue({
    revision: "revision",
    content: JSON.stringify({
      $schema: "openscp.map-resources", version: 1, fingerprint: "terrain",
      source: { packagePath: "SimCity_RegionTerrain0.package", group: "region" }, strokes: [],
    }),
  }) } },
}));
vi.mock("@/stores/gamePackages", () => ({ useGamePackagesStore: () => ({ opened: [] }) }));

it("keeps all map layers visible when selecting a city and feeds the shared preview resource edits", async () => {
  const coal = 0xd779c976;
  const data: Region3DData = {
    size: 2, metersPerPixel: 16, originWorld: [0, 0], heightPngBase64: "terrain",
    groundPngBase64: "ground", brushes: [], heightDiv: 32, heightBias: -1024,
    waterZ: -870, desert: false, displayName: "反影環礁", displayNameEn: "Reflection Atoll",
    plots: [
      { uid: "first", name: "石油灣", nameEn: "Oil Bay", kind: "city", x: 8, y: 8, z: -800 },
      { uid: "second", name: "格蘭港", nameEn: "Port", kind: "city", x: 24, y: 24, z: -800 },
      { uid: "work", name: null, kind: "greatwork", x: 16, y: 16, z: -800 },
    ],
  };
  vi.mocked(invoke).mockImplementation(async (command) => command === "map_panel_region_3d"
    ? data
    : { fingerprint: "terrain", cities: [{
      site: "first", environment: "first-env", bounds: [0, 0, 112, 112], windDirection: 90,
      maps: [{ id: coal, size: 7, values: Array(49).fill(0) }],
    }] });
  i18n.global.locale.value = "zh-CN";
  const wrapper = mount(MapResourceWorkbench, {
    attachTo: document.body,
    props: { project: "test", overview: false },
    global: { plugins: [i18n], stubs: { FSheet: true, MapViewer3D: true } },
  });
  try {
    await vi.waitFor(async () => {
      await flushPromises();
      expect(wrapper.findComponent({ name: "MapViewer3D" }).exists()).toBe(true);
    }, { timeout: 8000 });
    const viewer = wrapper.getComponent({ name: "MapViewer3D" });
    expect(viewer.props()).toMatchObject({
      data, showWater: true, showRoads: true, showVegetation: true, showPlots: true,
      activeResource: null, terrainEditing: false,
    });
    const cityMenu = wrapper.get("button[id$='-city']");
    await cityMenu.trigger("click");
    const slots = [...document.querySelectorAll<HTMLButtonElement>(".f-dropdown-panel [role=menuitemradio]")];
    expect(slots.map((slot) => slot.textContent?.trim())).toEqual(["石油灣", "格蘭港"]);
    slots[1]!.click();
    await nextTick();
    expect(viewer.props("selectedPlotUid")).toBe("second");
    viewer.vm.$emit("selectPlot", "first");
    await nextTick();
    expect(cityMenu.text()).toBe("石油灣");
    expect(viewer.props()).toMatchObject({ activeResource: null, selectedPlotUid: "first" });
    expect(wrapper.find("select, input[type=range]").exists()).toBe(false);

    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(cityMenu.text()).toBe("Oil Bay");
    const button = (label: string) => wrapper.findAll("button").find((item) => item.text() === label)!;
    await button("Paint").trigger("click");
    expect(viewer.props()).toMatchObject({ activeResource: "coal", resourceBrushDrag: true, terrainEditing: true });
    viewer.vm.$emit("resourceStroke", [[8, 8]]);
    await nextTick();
    const coalCells = () => viewer.props("resourceData").staticLayers.sources[0].maps[0].values.slice(0, 4);
    expect(coalCells()).toEqual([32768, 25891, 10114, 0]);
    await button("Eraser").trigger("click");
    viewer.vm.$emit("resourceStroke", [[8, 8]]);
    await nextTick();
    expect(coalCells()).toEqual([0, 0, 0, 0]);
    await button("Undo last stroke").trigger("click");
    expect(coalCells()).toEqual([32768, 25891, 10114, 0]);
    expect(button("Save edits").classes()).toContain("f-button-primary");
    await button("Wind direction").trigger("click");
    expect(viewer.props()).toMatchObject({ activeResource: "wind", brushRadius: 0, resourceBrushDrag: false });
    expect(viewer.props("resourceData").windArrows).toEqual([{ site: "first", x: 56, y: 33.6, direction: 90 }]);
    expect(wrapper.text()).not.toContain("Peak concentration");
    await button("Place arrow").trigger("click");
    wrapper.getComponent({ name: "FSlider" }).vm.$emit("update:modelValue", 217);
    viewer.vm.$emit("terrainPoint", 24, 40);
    await nextTick();
    expect(viewer.props("resourceData").windArrows).toEqual([{ site: "first", x: 24, y: 40, direction: 217 }]);
    expect(coalCells()).toEqual([32768, 25891, 10114, 0]);
    await button("Undo last stroke").trigger("click");
    expect(viewer.props("resourceData").windArrows[0].direction).toBe(90);
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = "zh-CN";
  }
});
