import { flushPromises, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { i18n } from "@/locales";
import type { Region3DData } from "@/lib/region-map";
import MapPreviewPane from "./MapPreviewPane.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn() }));

it("previews and exports the same regional stamp while labels follow the locale", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ heightPngBase64: "edited" })
    .mockResolvedValueOnce({ entryCount: 5 });
  vi.mocked(save).mockResolvedValue("mountain.package");
  i18n.global.locale.value = "zh-CN";
  const data: Region3DData = {
    size: 2, metersPerPixel: 16, originWorld: [0, 0], heightPngBase64: "original",
    groundPngBase64: "", plots: [], brushes: [], heightDiv: 32, heightBias: -1024,
    waterZ: -870, desert: false, displayName: null, displayNameEn: null,
  };
  const source = { packagePath: "terrain.package", group: "BEAF0510" };
  const wrapper = mount(MapPreviewPane, {
    props: { data, terrainSource: source },
    global: { plugins: [i18n], stubs: { FSheet: true, MapViewer3D: true } },
  });
  try {
    const button = (label: string) => wrapper.findAll("button").find((item) => item.text() === label)!;
    await button("抬升").trigger("click");
    expect(button("抬升").attributes("aria-pressed")).toBe("true");
    expect(wrapper.text()).toContain("中键旋转");
    const viewer = wrapper.findComponent({ name: "MapViewer3D" });
    expect(viewer.props("terrainEditing")).toBe(true);
    await wrapper.get('input[type="range"]').setValue("512");
    expect(viewer.props("brushRadius")).toBe(512);
    viewer.vm.$emit("terrainPoint", 2560, 1120);
    await flushPromises();
    const strokes = [{ mode: "raise", x: 2560, y: 1120, radius: 512, strength: 64 }];
    expect(invoke).toHaveBeenNthCalledWith(1, "map_panel_apply_terrain", { ...source, strokes });
    expect(viewer.props("editedHeight")).toBe("edited");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(button("Raise").attributes("aria-pressed")).toBe("true");
    expect(wrapper.text()).toContain("Middle drag to orbit");
    expect(wrapper.text()).toContain("Level uses the height");
    await button("Export overlay").trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenNthCalledWith(2, "map_panel_apply_terrain", {
      ...source, strokes, outPath: "mountain.package",
    });
    expect(wrapper.text()).toContain("Exported 5 terrain tiles.");
    await button("Undo").trigger("click");
    expect(viewer.props("editedHeight")).toBeNull();
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = "zh-CN";
  }
});
