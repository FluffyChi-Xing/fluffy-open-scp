import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { nextTick } from "vue";
import { describe, expect, it } from "vitest";
import { i18n } from "@/locales";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";
import type { ReplicaData } from "@/lib/game-ui/replica/types";
import MenuGroupingPanel from "./MenuGroupingPanel.vue";
import MenuMoveControl from "./MenuMoveControl.vue";
import FSelect from "@/components/ui/FSelect.vue";

describe("group management controls", () => {
  it("groups selected items, navigates to the group, and switches interface language", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useUiWorkbenchStore();
    store.hydrate({ model: { categories: [{ id: "11111111", label: "Roads", icon: null }] },
      tools: { "11111111": [{ instance: "AAAAAAAA", label: "Fire", preview: null, marquee: null }] },
    } as unknown as ReplicaData);
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(MenuGroupingPanel, { props: { menuId: "11111111" }, global: { plugins: [pinia, i18n] } });
    expect(wrapper.text()).toContain("分组管理");
    expect(wrapper.findAll('select')).toHaveLength(0);
    await wrapper.get('input[type="checkbox"]').setValue(true);
    await wrapper.get('input[maxlength]').setValue("Emergency");
    await wrapper.get('button.action').trigger('click');
    expect(store.menuGroups).toHaveLength(1);
    expect(store.menuEntries("11111111")).toEqual([]);
    expect(store.menuEntries(store.menuGroups[0].id)[0].tool.id).toBe("AAAAAAAA");
    expect(store.selectedMenuId).toBe("11111111");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("Group management");
    expect(wrapper.text()).not.toContain("分组管理");
    wrapper.unmount();
    i18n.global.locale.value = "zh-CN";
  });
  it("requires a move destination and emits a move rather than a copy", async () => {
    const wrapper = mount(MenuMoveControl, { props: { targets: [{ id: "11111111", label: "Roads" }] }, global: { plugins: [i18n] } });
    expect(wrapper.findAll('select')).toHaveLength(0);
    expect(wrapper.get('.move-control > button').attributes('disabled')).toBeDefined();
    wrapper.findComponent(FSelect).vm.$emit('update:modelValue', '11111111');
    await wrapper.vm.$nextTick();
    await wrapper.get('.move-control > button').trigger('click');
    expect(wrapper.emitted('move')).toEqual([["11111111"]]);
    expect(wrapper.emitted('copy')).toBeUndefined();
    wrapper.unmount();
  });
});
