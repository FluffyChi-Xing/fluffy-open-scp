import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { i18n } from "@/locales";
import Card from "./RuntimeLogsCard.vue";
describe("runtime overview", () => {
  it("shows saved game events only and updates language", async () => {
    const w = mount(Card, {
      props: {
        events: [
          {
            id: 1,
            level: "info",
            topic: "runtime.key_event",
            message: "raw",
            payload: { eventID: 42 },
            createdAt: 1,
          },
          {
            id: 2,
            level: "info",
            topic: "package.open",
            message: "package",
            createdAt: 2,
          },
        ],
      },
      global: {
        plugins: [i18n],
        stubs: { RouterLink: { template: "<a><slot /></a>" } },
      },
    });
    expect(w.find(".saved-summary strong").text()).toBe("1");
    expect(w.text()).toContain("游戏事件 42");
    expect(w.text()).not.toContain("package");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(w.text()).toContain("Game event 42");
    expect(w.text()).toContain("Open log console");
    await w.setProps({ events: [] });
    expect(w.text()).toContain("No observed events yet");
    i18n.global.locale.value = "zh-CN";
    w.unmount();
  });
});
