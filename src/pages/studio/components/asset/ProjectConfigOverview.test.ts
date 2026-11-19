import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import ProjectConfigOverview from "./ProjectConfigOverview.vue";
import { emptyMenuBinding } from "./menuBinding";
import { emptyLogicGraph } from "./logicGraph";
it("summarizes live edits read-only in collapsible sections and translates without losing state", async () => {
  const before = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const wrapper = mount(ProjectConfigOverview, {
    props: {
      manifest: {
        name: "Project A",
        version: "1.0",
        dependencies: { resources: { pathEntries: ["path-A"] } },
      },
      menu: { ...emptyMenuBinding(), label: "Menu A" },
      logic: emptyLogicGraph(),
      variant: false,
    },
    global: { plugins: [i18n] },
  });
  try {
    expect(wrapper.findAll("details")).toHaveLength(6);
    expect(
      wrapper.findAll('input,select,textarea,[contenteditable="true"]'),
    ).toHaveLength(0);
    expect(wrapper.text()).toContain("Project A");
    expect(wrapper.text()).toContain("Menu A");
    expect(wrapper.text()).toContain("path-A");
    await wrapper.setProps({
      menu: { ...emptyMenuBinding(), label: "Unsaved menu" },
      manifest: { name: "Project B" },
    });
    expect(wrapper.text()).toContain("Unsaved menu");
    expect(wrapper.text()).toContain("Project B");
    await wrapper.get("details").trigger("click");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("Basics");
    expect(wrapper.emitted()).not.toHaveProperty("update:menu");
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = before;
  }
});
