import { flushPromises, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import PackageProjectForm from "./PackageProjectForm.vue";
import { addLogicNode, connectLogic, emptyLogicGraph, ensureCoreNode, leafSpec, leafTypeOf } from "./logicGraph";
import { emptyMenuBinding } from "./menuBinding";

vi.mock("@/api", () => ({ tauriApi: { workspace: {
  codeReadText: vi.fn().mockResolvedValue({ content: '{"name":"Test building"}', revision: "one" }),
} } }));
vi.mock("@/api/tauri", () => ({ command: vi.fn().mockResolvedValue({ conflicts: [], unknownBundles: [] }) }));

it("lets an existing office and park add retail and housing directly from the behavior form", async () => {
  const { graph: initial, node: core } = ensureCoreNode(emptyLogicGraph(), [0, 0]);
  let graph = connectLogic(initial, core.id, "asset", "behavior").graph;
  for (const type of ["landvalue", "office", "park"] as const) {
    const added = addLogicNode(graph, leafSpec(type), [0, 0]);
    graph = connectLogic(added.graph, added.node.id, core.id, "leaf").graph;
  }
  const locale = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const wrapper = mount(PackageProjectForm, {
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
    const emittedGraph = () => wrapper.emitted<[typeof graph]>("update:logic")!.at(-1)![0];
    expect(wrapper.find(".ppf-behavior select").exists()).toBe(false);
    for (const type of ["commercial", "residence"] as const) {
      const add = wrapper.findAll(".ppf-add-leaves button").find(button => button.text() === i18n.global.t(leafSpec(type).labelKey))!;
      await add.trigger("click");
      graph = emittedGraph();
      const leaf = graph.nodes.find(node => leafTypeOf(graph, node) === type);
      expect(leaf).toBeDefined();
      expect(graph.edges).toEqual(expect.arrayContaining([expect.objectContaining({ source: leaf!.id, target: core.id })]));
      await wrapper.setProps({ logic: graph });
    }
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("one switch");
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = locale;
  }
});
