import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import { command } from "@/api/tauri";
import RoadPreview from "./RoadPreview.vue";

vi.mock("@/api/tauri", () => ({ command: vi.fn() }));

it("loads the selected native appearance and offers no futurized dirt road", async () => {
  vi.mocked(command).mockResolvedValue([20, {}]);
  const locale = i18n.global.locale.value;
  const wrapper = mount(RoadPreview, {
    props: {
      project: "roads",
      gamePackage: "build/SimCity_Game.package",
      recipe: "streetcar",
      streetlights: "original",
    },
    global: { plugins: [i18n], stubs: { RoadAppearancePreview: true } },
  });
  const load = () =>
    wrapper
      .getComponent({ name: "RoadAppearancePreview" })
      .props("loadRecipe")();
  try {
    i18n.global.locale.value = "zh-CN";
    await nextTick();
    expect(wrapper.text()).toContain("未来化");
    await load();
    expect(command).toHaveBeenLastCalledWith(
      "read_road_recipe_appearance",
      expect.objectContaining({ recipe: "streetcar", future: false }),
    );
    await wrapper.findAll("button")[1].trigger("click");
    await load();
    expect(command).toHaveBeenLastCalledWith(
      "read_road_recipe_appearance",
      expect.objectContaining({ recipe: "streetcar", future: true }),
    );
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("Futurized");
    await wrapper.setProps({ recipe: "dirt" });
    expect(wrapper.findAll("button")).toHaveLength(0);
    await load();
    expect(command).toHaveBeenLastCalledWith(
      "read_road_recipe_appearance",
      expect.objectContaining({ recipe: "dirt", future: false }),
    );
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = locale;
  }
});

it("reuses native assets while editing width and reloads them for the futuristic style", async () => {
  vi.mocked(command).mockClear();
  vi.mocked(command).mockResolvedValue([
    20,
    {
      instances: [],
      ribbons: {},
      sweeps: {},
      models: {},
      textures: {},
      unsupportedComponents: [],
    },
  ]);
  const strip = { id: "lane", kind: "forward", width: 4 } as const;
  const wrapper = mount(RoadPreview, {
    props: {
      project: "roads",
      gamePackage: "SimCity_Game.package",
      recipe: "lowStreet",
      streetlights: "original",
      draft: [strip],
    },
    global: { plugins: [i18n], stubs: { RoadAppearancePreview: true } },
  });
  const load = () =>
    wrapper
      .getComponent({ name: "RoadAppearancePreview" })
      .props("loadRecipe")();
  try {
    await load();
    await wrapper.setProps({ draft: [{ ...strip, width: 5 }] });
    await load();
    expect(command).toHaveBeenCalledTimes(1);
    await wrapper.findAll("button")[1].trigger("click");
    await load();
    expect(command).toHaveBeenLastCalledWith(
      "read_road_recipe_appearance",
      expect.objectContaining({
        recipe: "lowStreet",
        future: true,
        draftAssets: true,
      }),
    );
    expect(command).toHaveBeenCalledTimes(2);
  } finally {
    wrapper.unmount();
    vi.mocked(command).mockReset();
  }
});
