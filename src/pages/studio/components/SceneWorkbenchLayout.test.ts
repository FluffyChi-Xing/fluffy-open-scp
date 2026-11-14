import { mount } from "@vue/test-utils";
import { h } from "vue";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import SceneWorkbenchLayout from "./SceneWorkbenchLayout.vue";

it("keeps the scene split and allows the library to use the full right column", async () => {
  const wrapper = mount(SceneWorkbenchLayout, {
    global: { plugins: [i18n] },
    slots: {
      files: () => h("div", { "data-test": "files" }),
      viewport: () => h("div", { "data-test": "view" }),
      lower: () => h("div", { "data-test": "section" }),
      library: () => h("div", { "data-test": "library" }),
    },
  });
  try {
    expect(
      wrapper.get('[id="maw-tree"]').find('[data-test="files"]').exists(),
    ).toBe(true);
    expect(
      wrapper.get('[id="maw-viewport"]').find('[data-test="view"]').exists(),
    ).toBe(true);
    expect(
      wrapper
        .get('[id="maw-blueprint"]')
        .find('[data-test="section"]')
        .exists(),
    ).toBe(true);
    expect(
      wrapper.get('[id="maw-right"]').find('[data-test="library"]').exists(),
    ).toBe(true);
    expect(wrapper.find('[id="maw-right-inspector"]').exists()).toBe(false);
  } finally {
    wrapper.unmount();
  }
});

it("allows building configuration to replace the center and keeps the library and inspector split", () => {
  const wrapper = mount(SceneWorkbenchLayout, {
    global: { plugins: [i18n] },
    slots: {
      center: () => h("div", { "data-test": "config" }),
      library: () => h("div", { "data-test": "library" }),
      inspector: () => h("div", { "data-test": "inspector" }),
    },
  });
  try {
    expect(
      wrapper.get('[id="maw-center"]').find('[data-test="config"]').exists(),
    ).toBe(true);
    expect(wrapper.find('[id="maw-viewport"]').exists()).toBe(false);
    expect(
      wrapper
        .get('[id="maw-right-tabs"]')
        .find('[data-test="library"]')
        .exists(),
    ).toBe(true);
    expect(
      wrapper
        .get('[id="maw-right-inspector"]')
        .find('[data-test="inspector"]')
        .exists(),
    ).toBe(true);
  } finally {
    wrapper.unmount();
  }
});
