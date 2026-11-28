import { mount } from "@vue/test-utils";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import FSelect from "@/components/ui/FSelect.vue";
import UtilityConsumerFields from "./UtilityConsumerFields.vue";

it("edits both utility cycles without erasing the other value and updates translated labels", async () => {
  const previous = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  try {
    for (const kind of ["power", "water"] as const) {
      const wrapper = mount(UtilityConsumerFields, {
        props: { kind, value: { mode: "explicit", amount: 12, rate: 72 }, disabled: false },
        global: { plugins: [i18n] },
      });
      const inputs = wrapper.findAll("input");
      await inputs[0].setValue(25);
      expect(wrapper.emitted("update:value")?.at(-1)).toEqual([{ mode: "explicit", amount: 25, rate: 72 }]);
      await inputs[1].setValue(30);
      expect(wrapper.emitted("update:value")?.at(-1)).toEqual([{ mode: "explicit", amount: 12, rate: 30 }]);
      await inputs[1].setValue("");
      expect(wrapper.emitted("update:value")?.at(-1)).toEqual([{ mode: "explicit", amount: 12, rate: null }]);
      expect(wrapper.text()).toContain("消费间隔");
      i18n.global.locale.value = "en-US";
      await wrapper.vm.$nextTick();
      expect(wrapper.text()).toContain("Consumption interval");
      wrapper.getComponent(FSelect).vm.$emit("update:modelValue", "inherit");
      expect(wrapper.emitted("update:value")?.at(-1)).toEqual([{ mode: "inherit", amount: 12, rate: 72 }]);
      expect(wrapper.find('input[type="checkbox"]').exists()).toBe(false);
      wrapper.unmount();
      i18n.global.locale.value = "zh-CN";
    }
  } finally { i18n.global.locale.value = previous; }
});
