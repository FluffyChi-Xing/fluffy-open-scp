import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import PerformanceLibraryPanel from "./PerformanceLibraryPanel.vue";
import FSelect from "@/components/ui/FSelect.vue";
import { PERFORMANCE_PRESETS } from "./performanceBindings";
it("selects a performance before placing an anchor and translates the mounted controls", async () => {
  const before = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const panel = mount(PerformanceLibraryPanel, {
    props: { units: [], editEnabled: true, selectedUnitId: null },
    global: { plugins: [i18n] },
  });
  try {
    expect(panel.get(".anchor-preset").text()).toBe("锚点");
    panel
      .getComponent(FSelect)
      .vm.$emit("update:modelValue", PERFORMANCE_PRESETS[0].id);
    await nextTick();
    await panel.get(".anchor-preset").trigger("pointerdown");
    expect(panel.emitted("place")).toEqual([
      [{ performancePreset: PERFORMANCE_PRESETS[0].id }],
    ]);
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(panel.get(".anchor-preset").text()).toBe("Anchor");
    expect(panel.getComponent(FSelect).props("options")[1].label).toContain(
      "Willis Tower",
    );
  } finally {
    panel.unmount();
    i18n.global.locale.value = before;
  }
});
