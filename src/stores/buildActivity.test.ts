import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, getActivePinia, setActivePinia } from "pinia";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import type { BuildQueueEventPayload } from "@/api/tauri";
import { i18n } from "@/locales";
import NotificationsPanel from "@/components/notification/NotificationsPanel.vue";
import { useBuildActivityStore } from "./buildActivity";

const bus = vi.hoisted(() => ({
  listener: undefined as ((payload: BuildQueueEventPayload) => void) | undefined,
}));
vi.mock("@/api", () => ({ isTauri: () => true }));
vi.mock("@/api/tauri", () => ({
  subscribe: async (_event: string, listener: (payload: BuildQueueEventPayload) => void) => {
    bus.listener = listener;
    return () => { bus.listener = undefined; };
  },
}));

describe("buildActivity store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });
  afterEach(() => {
    bus.listener = undefined;
    i18n.global.locale.value = "zh-CN";
  });

  it("remembers project and latest job for Actions focus", () => {
    const store = useBuildActivityStore();
    store.rememberProject("D:/mods/hope", "大连希望大厦", "job-1");
    expect(store.lastProjectRoot).toBe("D:/mods/hope");
    expect(store.lastProjectLabel).toBe("大连希望大厦");
    expect(store.lastJobId).toBe("job-1");
  });

  it("updates one row through queued, running, stage completion and failure", async () => {
    const store = useBuildActivityStore();
    await store.startListening();
    const publish = bus.listener!;
    const job = { jobId: "job-1", label: "Tower", totalStages: 16 };
    publish({ ...job, status: "queued" });
    const rowId = store.notices[0].id;
    const wrapper = mount(NotificationsPanel, {
      global: { plugins: [getActivePinia()!, i18n] },
    });
    try {
      expect(wrapper.get(".notification-status").text()).toBe("排队中");
      expect(wrapper.find("progress").exists()).toBe(false);
      publish({ ...job, status: "running" });
      const event = {
        stage: "load-inputs", kind: { type: "finished" as const },
        timestampMs: 100, durationMs: 20, detail: null,
      };
      publish({ ...job, event });
      publish({ ...job, event });
      await nextTick();
      expect(wrapper.get("progress").attributes()).toMatchObject({ value: "1", max: "16" });
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(wrapper.get(".notification-status").text()).toBe("Building");
      expect(wrapper.get("progress").attributes("aria-label")).toBe("Build progress");

      publish({ ...job, status: "failed", error: "BUILD_QUEUE_TIMEOUT" });
      publish({ ...job, status: "queued" });
      publish({ ...job, event });
      await nextTick();
      expect(store.notices).toHaveLength(1);
      expect(store.notices[0]).toMatchObject({ id: rowId, status: "failed" });
      expect(store.unreadCount).toBe(1);
      expect(wrapper.get(".notification-status").text()).toBe("Failed");
      expect(wrapper.find("progress").exists()).toBe(false);
      expect(wrapper.get(".notification-detail").text()).toContain("45 minutes");
      expect(wrapper.get(".notifications-action").text()).toBe("Clear");
      await wrapper.get(".notifications-action").trigger("click");
      expect(wrapper.find(".notification-item").exists()).toBe(false);
    } finally {
      wrapper.unmount();
    }
  });

  it("does not restore cleared running jobs when their final event arrives", async () => {
    const store = useBuildActivityStore();
    await store.startListening();
    const publish = bus.listener!;
    publish({ jobId: "cleared", status: "running" });
    store.clear();
    publish({ jobId: "cleared", status: "finished" });
    expect(store.notices).toEqual([]);
    expect(store.unreadCount).toBe(0);
    publish({ jobId: "new", status: "queued" });
    publish({ jobId: "new", status: "finished" });
    expect(store.notices).toHaveLength(1);
    expect(store.notices[0]).toMatchObject({ jobId: "new", status: "finished" });
  });
});
