import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useBuildActivityStore } from "./buildActivity";

describe("buildActivity store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("remembers project and latest job for Actions focus", () => {
    const store = useBuildActivityStore();
    store.rememberProject("D:/mods/hope", "大连希望大厦", "job-1");
    expect(store.lastProjectRoot).toBe("D:/mods/hope");
    expect(store.lastProjectLabel).toBe("大连希望大厦");
    expect(store.lastJobId).toBe("job-1");
  });

  it("dedupes identical lifecycle notices for the same job", () => {
    const store = useBuildActivityStore();
    store.pushNotice({
      titleKey: "notification.buildQueued",
      detail: "job-1",
      tone: "info",
      jobId: "job-1",
    });
    store.pushNotice({
      titleKey: "notification.buildQueued",
      detail: "job-1",
      tone: "info",
      jobId: "job-1",
    });
    expect(store.notices).toHaveLength(1);
    store.pushNotice({
      titleKey: "notification.buildFinished",
      detail: "job-1",
      tone: "success",
      jobId: "job-1",
    });
    expect(store.notices).toHaveLength(2);
    expect(store.unreadCount).toBe(2);
  });
});
