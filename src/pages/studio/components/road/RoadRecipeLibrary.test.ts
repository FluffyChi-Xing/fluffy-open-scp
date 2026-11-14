import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import RoadRecipeLibrary from "./RoadRecipeLibrary.vue";
import RoadCrossSection from "./RoadCrossSection.vue";

it("localizes the active recipe library and cross-section and keeps their controls usable", async () => {
  const previousLocale = i18n.global.locale.value;
  const plugins = [i18n];
  i18n.global.locale.value = "zh-CN";
  const library = mount(RoadRecipeLibrary, {
    props: { selected: "highStreet", streetlights: "original" },
    global: { plugins },
  });
  const section = mount(RoadCrossSection, {
    props: { recipe: "highStreet", streetlights: "original" },
    global: { plugins },
  });
  try {
    expect(library.text()).toContain("原版道路配方");
    expect(section.text()).toContain("人行道");
    expect(section.findAll(".part.traffic")).toHaveLength(4);
    await library.findAll("fieldset button")[2].trigger("click");
    expect(library.emitted("lighting")).toEqual([["right"]]);

    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(library.text()).toContain("Original road recipes");
    expect(section.text()).toContain("Sidewalk");
    await library.findAll("button")[5].trigger("click");
    expect(library.emitted("select")).toEqual([["highAvenue"]]);

    await section.setProps({ recipe: "highAvenue" });
    expect(section.findAll(".part.traffic")).toHaveLength(6);
  } finally {
    library.unmount();
    section.unmount();
    i18n.global.locale.value = previousLocale;
  }
});
