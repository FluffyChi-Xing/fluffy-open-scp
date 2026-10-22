import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { i18n } from "@/locales";
import PropertyMaterialInput from "./PropertyMaterialInput.vue";

describe("Property material input", () => {
  it("updates labels and existing errors when switching language", async () => {
    const previous = i18n.global.locale.value;
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(PropertyMaterialInput, {
      props: {
        modelValue: "bad",
        loading: false,
        errorKey: "propertyInvalidTgi",
      },
      global: { plugins: [i18n] },
    });
    try {
      expect(wrapper.text()).toContain("建筑材质");
      expect(wrapper.text()).toContain("请输入完整 Property TGI");
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(wrapper.text()).toContain("Building material");
      expect(wrapper.text()).toContain("Enter a full Property TGI");
      expect(wrapper.find("input").attributes("aria-invalid")).toBe("true");
      await wrapper.find("input").setValue("00b1b104:61f0c000:1d261fdd");
      expect(wrapper.emitted("update:modelValue")?.[0]).toEqual([
        "00b1b104:61f0c000:1d261fdd",
      ]);
    } finally {
      wrapper.unmount();
      i18n.global.locale.value = previous;
    }
  });
});
