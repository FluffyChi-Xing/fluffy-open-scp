import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import AnimatedPropertyViewer from "./AnimatedPropertyViewer.vue";
import { listAnimations } from "./animationPreview";

vi.mock("./animationPreview", () => ({ listAnimations: vi.fn() }));

const coal = { typeId: 0x00b1b104, group: 0x40e1c000, instance: 0x34e0e232 };

describe("building animation branch", () => {
  it("opens only for eligible assets, translates in place, and resets on asset changes", async () => {
    vi.mocked(listAnimations)
      .mockResolvedValueOnce([{ id: 0x2a72b9a8, resourceId: 0xcec5890c, start: 1, end: 0 }])
      .mockResolvedValueOnce([]);
    const wrapper = mount(AnimatedPropertyViewer, {
      props: { packageId: 1, tgi: coal, open: false },
      global: {
        plugins: [i18n],
        stubs: {
          PropertyViewer: {
            template: '<div><header><slot name="toolbar" /><div class="render-mode"><button /><button data-refined /></div></header><slot name="canvas-before" /><div data-static /></div>',
          },
          AnimationViewport: { props: ["refined"], template: '<div data-animation :data-refined-state="String(refined)" />' },
        },
      },
    });
    const previous = i18n.global.locale.value;
    try {
      expect(listAnimations).not.toHaveBeenCalled();
      await wrapper.setProps({ open: true });
      await flushPromises();
      i18n.global.locale.value = "zh-CN";
      await wrapper.vm.$nextTick();
      expect(wrapper.get('[role="switch"]').attributes("aria-label")).toBe("动画预览");
      expect(wrapper.find("[data-animation]").exists()).toBe(false);
      const original = wrapper.get("[data-static]").element;
      await wrapper.get('[role="switch"]').trigger("click");
      await flushPromises();
      expect(wrapper.find("[data-animation]").exists()).toBe(true);
      expect(wrapper.get("[data-animation]").attributes("data-refined-state")).toBe("false");
      wrapper.get("[data-refined]").element.classList.add("active");
      await vi.waitFor(() => {
        expect(wrapper.get("[data-animation]").attributes("data-refined-state")).toBe("true");
      });
      i18n.global.locale.value = "en-US";
      await wrapper.vm.$nextTick();
      expect(wrapper.get('[role="switch"]').attributes("aria-label")).toBe("Animation preview");
      expect(wrapper.find("[data-animation]").exists()).toBe(true);
      await wrapper.setProps({ tgi: { ...coal, instance: 2 } });
      await flushPromises();
      expect(wrapper.find('[role="switch"]').exists()).toBe(false);
      expect(wrapper.find("[data-animation]").exists()).toBe(false);
      expect(wrapper.get("[data-static]").element).toBe(original);
    } finally {
      wrapper.unmount();
      i18n.global.locale.value = previous;
    }
  });
});
