import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import FSelect from "@/components/ui/FSelect.vue";
import LogicNodeCard from "./LogicNodeCard.vue";
import { emptyLogicGraph, ensureCoreNode } from "./logicGraph";

it("updates the active composition selector when changing languages and emits the chosen mode", async () => {
  const core = ensureCoreNode(emptyLogicGraph(), [0, 0]);
  const previous = i18n.global.locale.value;
  i18n.global.locale.value = "en-US";
  const wrapper = mount(LogicNodeCard, {
    props: { node: core.node, params: core.graph.params[core.node.id], disabled: false, issues: [] },
    global: { plugins: [i18n], stubs: { FIcon: true, BlueprintPortRow: true } },
  });
  try {
    expect(wrapper.text()).toContain("Behavior composition");
    expect(wrapper.findAllComponents(FSelect)[0].props("options")).toContainEqual({
      value: "independent-v1", label: "Independent composition (experimental)",
    });
    i18n.global.locale.value = "zh-CN";
    await nextTick();
    expect(wrapper.text()).toContain("行为组合方式");
    expect(wrapper.findAllComponents(FSelect)[0].props("options")).toContainEqual({
      value: "independent-v1", label: "独立组合（实验）",
    });
    wrapper.findAllComponents(FSelect)[0].vm.$emit("update:modelValue", "independent-v1");
    expect(wrapper.emitted("update-params")?.at(-1)).toEqual([{ composition: "independent-v1" }]);
  } finally { wrapper.unmount(); i18n.global.locale.value = previous; }
});
