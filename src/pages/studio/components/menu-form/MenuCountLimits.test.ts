import { mount, flushPromises } from "@vue/test-utils";
import { createPinia } from "pinia";
import { nextTick } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import MenuCountLimits from "./MenuCountLimits.vue";
import manifest from "../../../../../public/game-ui/tools/count-limits.json";
import type { CountLimitDraft } from "@/lib/game-ui/count-limits";

describe("count limits sheet", () => {
  beforeEach(() => {
    i18n.global.locale.value = "zh-CN";
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => manifest }));
  });
  it("shows only supported fields and updates all text when switching languages", async () => {
    const wrapper = mount(MenuCountLimits, {
      props: { source: "BC45E41B", modelValue: null as CountLimitDraft | null,
        "onUpdate:modelValue": (value: CountLimitDraft | null) => { void wrapper.setProps({ modelValue: value }); } },
      global: { plugins: [createPinia(), i18n] },
    });
    await flushPromises();
    expect(wrapper.findAll('input[type="number"]')).toHaveLength(1);
    expect(wrapper.text()).toContain("单项建造上限");
    await wrapper.setProps({ source: "C0E5BE77" });
    await flushPromises();
    expect(wrapper.findAll('input[type="number"]')).toHaveLength(2);
    expect(wrapper.findAll('input').map((i) => (i.element as HTMLInputElement).value)).toEqual(["1", "3"]);
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("Shared limit for this group");
    await wrapper.find('input').setValue(1.5);
    expect(wrapper.find('[role="alert"]').text()).toContain("whole numbers");
    await wrapper.setProps({ source: "FFFFFFFF" });
    await flushPromises();
    expect(wrapper.find('section').exists()).toBe(false);
    wrapper.unmount();
  });
});
