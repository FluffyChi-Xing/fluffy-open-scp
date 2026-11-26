import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import PackageProjectForm from "./PackageProjectForm.vue";
import MegaTowerBehaviorForm from "./MegaTowerBehaviorForm.vue";
import GarbageBehaviorFields from "./GarbageBehaviorFields.vue";
import FSelect from "@/components/ui/FSelect.vue";
import { addLogicNode, connectLogic, emptyLogicGraph, ensureCoreNode, leafSpec, leafTypeOf } from "./logicGraph";
import { emptyMenuBinding } from "./menuBinding";

vi.mock("@/api", () => ({ tauriApi: { workspace: {
  codeReadText: vi.fn().mockResolvedValue({ content: '{"name":"Test building"}', revision: "one" }),
} } }));
vi.mock("@/api/tauri", () => ({ command: vi.fn().mockResolvedValue({ conflicts: [], unknownBundles: [] }) }));

it("lets an existing office and park add retail and housing directly from the behavior form", async () => {
  const { graph: initial, node: core } = ensureCoreNode(emptyLogicGraph(), [0, 0]);
  let graph = connectLogic(initial, core.id, "asset", "behavior").graph;
  for (const type of ["office", "park"] as const) {
    const added = addLogicNode(graph, leafSpec(type), [0, 0]);
    graph = connectLogic(added.graph, added.node.id, core.id, "leaf").graph;
  }
  const locale = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const wrapper = mount(PackageProjectForm, {
    attachTo: document.body,
    props: {
      project: "test-project", menu: emptyMenuBinding(), logic: graph, savedDirty: false,
      saveDocument: vi.fn().mockResolvedValue(true), documentPath: "asset.lot.json",
    },
    global: { plugins: [i18n], stubs: { MonacoEditor: true, ShiftScheduleSlider: true, ProjectConfigOverview: true } },
  });
  try {
    await flushPromises();
    const behaviorTab = wrapper.findAll('[role="tab"]').find(tab => tab.text() === i18n.global.t("studio.assetWorkbench.projectConfig.tabBehavior"))!;
    await behaviorTab.trigger("click");
    const recipe = wrapper.getComponent(MegaTowerBehaviorForm);
    const enabled = recipe.get('[role="switch"]');
    expect(enabled.attributes("aria-checked")).toBe("false");
    await enabled.trigger("click");
    expect(wrapper.emitted("update:megaTower")?.at(-1)).toEqual([{ floors: [{ kind: "commercial" }] }]);
    await wrapper.setProps({ megaTower: { floors: [{ kind: "commercial" }] } });
    expect(enabled.attributes("aria-checked")).toBe("true");
    expect(recipe.findAll('[role="switch"]')).toHaveLength(8);
    expect(recipe.find('input[type="checkbox"]').exists()).toBe(false);
    await recipe.findAll('[role="switch"]')
      .find(toggle => toggle.attributes("aria-label") === i18n.global.t("studio.assetWorkbench.megaTower.office"))!
      .trigger("click");
    expect(wrapper.emitted("update:megaTower")?.at(-1)).toEqual([{ floors: [{ kind: "commercial" }, { kind: "office" }] }]);
    await enabled.trigger("click");
    expect(wrapper.emitted("update:megaTower")?.at(-1)).toEqual([undefined]);
    await wrapper.setProps({ megaTower: undefined });
    const emittedGraph = () => wrapper.emitted<[typeof graph]>("update:logic")!.at(-1)![0];
    expect(wrapper.find(".ppf-behavior select").exists()).toBe(false);
    const garbage = wrapper.getComponent(GarbageBehaviorFields);
    const select = garbage.getComponent(FSelect);
    expect(garbage.find("input").exists()).toBe(false);
    select.vm.$emit("update:modelValue", "explicit");
    await wrapper.setProps({ logic: emittedGraph() });
    await garbage.get("input").setValue("3");
    const updated = emittedGraph();
    expect(updated.params[core.id]).toEqual({
      ...graph.params[core.id],
      garbage: { mode: "explicit", amount: 3, rate: null },
    });
    expect(updated.edges).toEqual(graph.edges);
    graph = updated;
    await wrapper.setProps({ logic: graph });
    const leafButton = (label: string) => wrapper.findAll<HTMLButtonElement>(".ppf-add-leaves button")
      .find(button => button.text() === label)!;
    expect(leafButton("住宿").element.disabled).toBe(false);
    expect(leafButton("博彩").element.disabled).toBe(true);
    expect(leafButton("ControlNet 生产").element.disabled).toBe(true);
    const previousUpdates = wrapper.emitted("update:logic")!.length;
    await leafButton("博彩").trigger("click");
    await leafButton("ControlNet 生产").trigger("click");
    expect(wrapper.emitted("update:logic")).toHaveLength(previousUpdates);
    for (const type of ["commercial", "residence"] as const) {
      const add = wrapper.findAll(".ppf-add-leaves button").find(button => button.text() === i18n.global.t(leafSpec(type).labelKey))!;
      await add.trigger("click");
      graph = emittedGraph();
      const leaf = graph.nodes.find(node => leafTypeOf(graph, node) === type);
      expect(leaf).toBeDefined();
      expect(graph.edges).toEqual(expect.arrayContaining([expect.objectContaining({ source: leaf!.id, target: core.id })]));
      await wrapper.setProps({ logic: graph });
    }
    expect(leafButton("住宿").element.disabled).toBe(true);
    expect(wrapper.text()).toContain("每栋建筑最多可添加 4 个行为叶子。");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("one switch");
    expect(leafButton("Gambling").element.disabled).toBe(true);
    expect(leafButton("ControlNet production").element.disabled).toBe(true);
    expect(wrapper.text()).toContain("A building can have up to 4 behavior leaves.");
    expect(garbage.text()).toContain("Garbage generation amount");
    const base = connectLogic(initial, core.id, "asset", "behavior").graph;
    await wrapper.setProps({ logic: base });
    expect(leafButton("ControlNet production").element.disabled).toBe(false);
    await leafButton("ControlNet production").trigger("click");
    const stationGraph = emittedGraph();
    const station = stationGraph.nodes.find(node => leafTypeOf(stationGraph, node) === "controlnet")!;
    expect(stationGraph.params[station.id]).toEqual({ type: "controlnet" });
    expect(stationGraph.edges).toContainEqual(expect.objectContaining({ source: station.id, target: core.id }));
    await wrapper.setProps({ logic: stationGraph });
    expect(leafButton("Hotel").element.disabled).toBe(true);
    await wrapper.setProps({ logic: base });
    await leafButton("Fusion power").trigger("click");
    const fusionGraph = emittedGraph();
    const fusion = fusionGraph.nodes.find(node => leafTypeOf(fusionGraph, node) === "fusion-power")!;
    await wrapper.setProps({ logic: fusionGraph });
    expect(leafButton("Hotel").element.disabled).toBe(true);
    await wrapper.get('button[aria-haspopup="dialog"]').trigger("click");
    await flushPromises();
    const sheet = new DOMWrapper(document.body).get(".behavior-sheet");
    const toggle = sheet.get('[role="switch"]');
    expect(toggle.attributes("aria-checked")).toBe("true");
    await toggle.trigger("click");
    expect(toggle.attributes("aria-checked")).toBe("false");
    expect(sheet.text()).toContain("local readiness");
    await sheet.get("input").setValue("0");
    expect(sheet.get("button[type=submit]").attributes("disabled")).toBeDefined();
    await sheet.get("input").setValue("432100");
    i18n.global.locale.value = "zh-CN";
    await nextTick();
    expect(toggle.attributes("aria-label")).toBe("需要外部 ControlNet");
    await sheet.get("form").trigger("submit");
    expect(emittedGraph().params[fusion.id]).toEqual({
      type: "fusion-power", requiresControlNet: false, chargeCapacity: 432100,
    });
    i18n.global.locale.value = "en-US";
    const tourism = addLogicNode(base, leafSpec("tourism"), [0, 0]);
    const tourismGraph = connectLogic(tourism.graph, tourism.node.id, core.id, "leaf").graph;
    await wrapper.setProps({ logic: tourismGraph });
    expect(leafButton("Gambling").element.disabled).toBe(false);
    await leafButton("Gambling").trigger("click");
    const withCasino = emittedGraph();
    const casino = withCasino.nodes.find(node => leafTypeOf(withCasino, node) === "casino")!;
    expect(withCasino.params[casino.id]).toEqual({ type: "casino" });
    expect(withCasino.edges).toContainEqual(expect.objectContaining({ source: casino.id, target: core.id }));
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = locale;
  }
});
