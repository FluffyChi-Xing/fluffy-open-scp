import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import SpawnerLibraryPanel from "./SpawnerLibraryPanel.vue";

it("offers one anchor and translates the mounted placement control", async () => {
  const previousLocale = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const panel = mount(SpawnerLibraryPanel, {
    global: { plugins: [i18n] },
    props: { units: [], editEnabled: true },
  });
  try {
    expect(panel.findAll("button")).toHaveLength(1);
    const anchor = panel.get("button");
    expect(anchor.text()).toBe("通用锚点");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(anchor.text()).toBe("General anchor");
    await anchor.trigger("pointerdown");
    expect(panel.emitted("place")).toEqual([[]]);
  } finally {
    panel.unmount();
    i18n.global.locale.value = previousLocale;
  }
});
