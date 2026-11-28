import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import RoadDraftSection from "./RoadDraftSection.vue";
import RoadStripLibrary from "./RoadStripLibrary.vue";
import { moveStripDragType, stripDragType, type RoadStrip } from "./roadDraft";
import {
  matchingRecipe,
  parseRoadSchema,
  recipeDraft,
  recipeIds,
} from "./roadSchema";

it("builds from empty with repeated dividers, width edits, and drag reordering", async () => {
  const plugins = [i18n];
  const originalLocale = i18n.global.locale.value;
  const editor = mount(RoadDraftSection, {
    props: { strips: [] },
    global: { plugins },
  });
  const library = mount(RoadStripLibrary, { global: { plugins } });
  const writes = new Map<string, string>();
  const dataTransfer = {
    setData: (type: string, value: string) => writes.set(type, value),
    getData: (type: string) => writes.get(type) ?? "",
  };
  const latest = () => editor.emitted("update")?.at(-1)?.[0] as RoadStrip[];
  const insert = async (data: string, gap: number) => {
    writes.clear();
    const card = library
      .findAll("button")
      .find((button) => button.text().includes(data));
    expect(card).toBeDefined();
    await card?.trigger("dragstart", { dataTransfer });
    const target = editor.findAll(".gap")[gap] ?? editor.get(".empty");
    await target.trigger("drop", { dataTransfer });
    await editor.setProps({ strips: latest() });
  };
  try {
    i18n.global.locale.value = "zh-CN";
    await nextTick();
    expect(editor.findAll(".strip")).toHaveLength(0);
    expect(editor.get(".empty").text()).toContain("将第一个组件拖到这里");
    await insert("分隔带", 0);
    expect(writes.get(stripDragType)).toBe("median");
    await insert("正向汽车道", 1);
    await insert("分隔带", 2);
    expect(latest().map((strip) => strip.kind)).toEqual([
      "median",
      "forward",
      "median",
    ]);
    expect(new Set(latest().map((strip) => strip.id)).size).toBe(3);
    await editor.findAll("input")[2].setValue("4.25");
    await editor.setProps({ strips: latest() });
    expect(latest()[2].width).toBe(4.25);
    writes.clear();
    await editor
      .findAll("article strong")[2]
      .trigger("dragstart", { dataTransfer });
    expect(writes.get(moveStripDragType)).toBe(latest()[2].id);
    await editor.get(".gap").trigger("drop", { dataTransfer });
    expect(latest().map((strip) => strip.width)).toEqual([4.25, 2, 3.2]);
    await editor.setProps({ strips: latest() });
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(editor.text()).toContain("Forward vehicle lane");
    expect(library.text()).toContain("Street trees");
  } finally {
    editor.unmount();
    library.unmount();
    i18n.global.locale.value = originalLocale;
  }
});

it("round-trips empty and authored drafts and resolves only matching templates", () => {
  const base = {
    format: "openscp.road-asset/1",
    title: "Workshop road",
    description: "",
    recipe: "highStreet",
    identity: 1,
    gamePackage: "",
    scriptsPackage: "",
  };
  const section = recipeDraft("mediumAvenue");
  expect(
    parseRoadSchema(JSON.stringify({ ...base, section: [] })).section,
  ).toEqual([]);
  expect(parseRoadSchema(JSON.stringify({ ...base, section })).section).toEqual(
    section,
  );
  expect(parseRoadSchema(JSON.stringify(base)).section).toBeUndefined();
  for (const recipe of recipeIds)
    expect(matchingRecipe(recipeDraft(recipe))).toBe(recipe);
  expect(
    matchingRecipe([...section, { id: "extra", kind: "median", width: 2 }]),
  ).toBeUndefined();
});
