import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import PropertyPreview from "./PropertyPreview.vue";

vi.mock("@/api", () => ({
  tauriApi: { packages: { readLotEditorSession: async () => null } },
}));

it("opens road appearance 3D from the property table and resets on resource change", async () => {
  const locale = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const preview = {
    kind: "property" as const,
    packageId: 1,
    tgi: { typeId: 0x00b1b104, group: 0x09558b7e, instance: 0xdc6c325f },
    entries: [],
    offset: 0,
    bytes: [],
    totalLength: 0,
    claimedCount: 0,
  };
  const wrapper = mount(PropertyPreview, {
    props: { preview },
    global: {
      plugins: [i18n],
      stubs: {
        PropertyViewer: true,
        RoadAppearancePreview: true,
        FSheet: true,
      },
    },
  });
  const toggle = () => wrapper.findAll("button").at(-1)!;
  try {
    expect(wrapper.find("table").exists()).toBe(true);
    expect(toggle().text()).toContain("3D 预览");
    await toggle().trigger("click");
    expect(wrapper.find("road-appearance-preview-stub").exists()).toBe(true);
    expect(wrapper.find("table").exists()).toBe(false);
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(toggle().text()).toContain("Properties");
    await toggle().trigger("click");
    expect(wrapper.find("table").exists()).toBe(true);
    await toggle().trigger("click");
    await wrapper.setProps({
      preview: { ...preview, tgi: { ...preview.tgi, instance: 2 } },
    });
    expect(wrapper.find("table").exists()).toBe(true);
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = locale;
  }
});
