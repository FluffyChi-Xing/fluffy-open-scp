import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import PropertyViewer from "./PropertyViewer.vue";

describe("PropertyViewer", () => {
  it("always forwards the viewer mode and keeps extension slots available", () => {
    const wrapper = mount(PropertyViewer, {
      props: { packageId: 1, tgi: { typeId: 1, group: 2, instance: 3 } },
      global: { stubs: { PropertyEditor: { template: "<div :data-mode='mode'><slot name='toolbar' /><slot name='tabs' /></div>", props: ["mode"] } } },
      slots: { toolbar: "toolbar slot", tabs: "tabs slot" },
    });
    expect(wrapper.text()).toContain("toolbar slot");
    expect(wrapper.text()).toContain("tabs slot");
    expect(wrapper.get("[data-mode='viewer']").element).toBeTruthy();
  });
});
