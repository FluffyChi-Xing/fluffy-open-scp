import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import RoadComponentPalette from "./RoadComponentPalette.vue";
import RoadCrossSection from "./RoadCrossSection.vue";
import { roadComponentDragType } from "./roadComposition";
import { parseRoadSchema } from "./roadSchema";

it("drops a component only on its compatible slot and switches the visible labels", async () => {
  const locale = i18n.global.locale.value;
  const palette = mount(RoadComponentPalette, {
    props: { recipe: "mediumStreet" },
    global: { plugins: [i18n] },
  });
  const section = mount(RoadCrossSection, {
    props: { recipe: "mediumStreet", streetlights: "original" },
    global: { plugins: [i18n] },
  });
  const data = new Map<string, string>();
  const dataTransfer = {
    getData: (type: string) => data.get(type) ?? "",
    setData: (type: string, value: string) => data.set(type, value),
  };
  try {
    i18n.global.locale.value = "zh-CN";
    await nextTick();
    expect(palette.text()).toContain("左侧行道树");
    const source = palette.get("button");
    await source.trigger("dragstart", { dataTransfer });
    expect(data.get(roadComponentDragType)).toBe("leftTrees");
    await section.get("button.rightTrees").trigger("drop", { dataTransfer });
    expect(section.emitted("addComponent")).toBeUndefined();
    await section.get("button.leftTrees").trigger("drop", { dataTransfer });
    expect(section.emitted("addComponent")).toEqual([["leftTrees"]]);
    await section.setProps({ composition: { leftTrees: true } });
    expect(section.get("button.leftTrees").attributes("aria-pressed")).toBe(
      "true",
    );
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(section.text()).toContain("Left-side street trees");
    await section.setProps({ recipe: "streetcar" });
    expect(section.find("button.tram").exists()).toBe(false);
  } finally {
    section.unmount();
    palette.unmount();
    i18n.global.locale.value = locale;
  }
});

it("preserves composition on reload and rejects unsupported combinations", () => {
  const schema = {
    format: "openscp.road-asset/1",
    recipe: "mediumStreet",
    identity: 1,
    title: "Road",
    description: "",
    gamePackage: "SimCity_Game.package",
    scriptsPackage: "SimCity-Scripts.package",
    composition: { leftTrees: true, rightTrees: true, tram: true },
  };
  expect(parseRoadSchema(JSON.stringify(schema))).toEqual(schema);
  expect(() =>
    parseRoadSchema(JSON.stringify({ ...schema, recipe: "streetcar" })),
  ).toThrow("road_schema_invalid");
});
