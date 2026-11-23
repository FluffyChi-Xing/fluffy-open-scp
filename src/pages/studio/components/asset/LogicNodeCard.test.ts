import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import LogicNodeCard from "./LogicNodeCard.vue";
import { emptyLogicGraph, ensureCoreNode } from "./logicGraph";

it("explains the shared building switch in both languages", async () => {
  const core = ensureCoreNode(emptyLogicGraph(), [0, 0]);
  const previous = i18n.global.locale.value;
  i18n.global.locale.value = "en-US";
  const wrapper = mount(LogicNodeCard, {
    props: { node: core.node, params: core.graph.params[core.node.id], disabled: false, issues: [] },
    global: { plugins: [i18n], stubs: { FIcon: true, BlueprintPortRow: true } },
  });
  try {
    expect(wrapper.text()).toContain("one switch");
    i18n.global.locale.value = "zh-CN";
    await nextTick();
    expect(wrapper.text()).toContain("整栋只有一个开关");
  } finally { wrapper.unmount(); i18n.global.locale.value = previous; }
});
