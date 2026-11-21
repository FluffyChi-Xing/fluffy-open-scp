import { mount } from "@vue/test-utils";
import { reactive, nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import EditorPackagesBar from "./EditorPackagesBar.vue";
it("updates package status and directory controls when switching languages", async () => {
  const scope = reactive({
    opened: [{ package: { packageId: 1, path: "Game/SimCity_Game.package" } }],
    directory: "Game",
    loading: false,
    missing: ["SimCityDataEP1.package"],
    failed: false,
    ready: Promise.resolve(),
    load: vi.fn(),
    openFile: vi.fn(),
  });
  const previous = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const wrapper = mount(EditorPackagesBar, {
    props: { scope },
    global: { plugins: [i18n] },
  });
  try {
    expect(wrapper.text()).toContain("已加载 1 个资源包");
    expect(wrapper.text()).toContain("其余可用资源仍可使用");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("1 resource packages loaded");
    expect(wrapper.text()).toContain("Available resources remain usable");
    expect(wrapper.text()).toContain("Choose folder");
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = previous;
  }
});
