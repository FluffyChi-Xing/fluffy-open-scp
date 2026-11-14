import { mount, flushPromises } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import Page from "./index.vue";
import LogStream from "@/components/runtime-observer/LogStream.vue";
import { runtimeObserver } from "@/api/runtimeObserver";
vi.mock("@/api/runtimeObserver", () => ({
  runtimeObserver: {
    listProcesses: vi.fn(),
    status: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
    sync: vi.fn(),
  },
}));
const idle = {
  running: false,
  records: 0,
  synced: 0,
  recent: [],
  eventCounts: {},
};
afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});
describe("observer controls", () => {
  it("switches the building catalog between city and development menus", async () => {
    vi.mocked(runtimeObserver.listProcesses).mockResolvedValue([{ pid: 4, name: "SimCity.exe" }]);
    vi.mocked(runtimeObserver.status).mockResolvedValue(idle);
    vi.mocked(runtimeObserver.stop).mockResolvedValue(idle);
    vi.mocked(runtimeObserver.start).mockResolvedValue({ ...idle, running: true });
    const w = mount(Page, { global: { plugins: [i18n] } });
    try {
      await flushPromises();
      const click = (label: string) =>
        w.findAll("button").find((button) => button.text() === label)!.trigger("click");
      await click("建筑观测");
      expect(w.text()).toContain("燃煤发电厂");
      await click("城市发展");
      expect(w.text()).toContain("OmegaCo 工厂");
      expect(w.text()).not.toContain("燃煤发电厂");
      await click("城市");
      await click("燃油发电厂");
      await click("开始捕获");
      await flushPromises();
      expect(runtimeObserver.start).toHaveBeenCalledWith(4, 60, "balanced", "city:1848207142");
    } finally {
      w.unmount();
    }
  });

  it("distinguishes a stationary clock from a confirmed pause", async () => {
    vi.mocked(runtimeObserver.listProcesses).mockResolvedValue([]);
    vi.mocked(runtimeObserver.status).mockResolvedValue({
      ...idle, records: 3, clockAdvancing: false, gameClockSeconds: 64800,
    });
    vi.mocked(runtimeObserver.stop).mockResolvedValue(idle);
    const w = mount(Page, { global: { plugins: [i18n] } });
    try {
      await flushPromises();
      expect(w.get(".runtime-pill").text()).toBe("时钟未推进");
      expect(w.get(".runtime-time").text()).toContain("最后采样时间");
      expect(w.get(".runtime-time").text()).toContain("18:00:00");
      i18n.global.locale.value = "en-US";
      await flushPromises();
      expect(w.get(".runtime-pill").text()).toBe("Clock stationary");
    } finally {
      i18n.global.locale.value = "zh-CN";
      w.unmount();
    }
  });

  it("gives vehicle and worker rows separate categories and shows inventory changes", async () => {
    const w = mount(LogStream, {
      props: {
        rows: [
          { sequence: 1, channel: "asset.resource", metric: "vehicles", name: "carW1",
            unitSlot: 4, unitStamp: 7, amountDelta: 2,
            before: { amount: 0, capacity: 12 }, after: { amount: 2, capacity: 12 } },
          { sequence: 2, channel: "asset.resource", metric: "workers", name: "totalWorkers",
            unitSlot: 4, unitStamp: 7, before: null, after: { amount: 5, capacity: 12 } },
        ],
      },
      global: { plugins: [i18n] },
    });
    try {
      expect(w.get(".tone-vehicles .line-kind").text()).toBe("车辆");
      expect(w.get(".tone-workers .line-kind").text()).toBe("劳工");
      expect(w.get(".tone-vehicles").text()).toContain("库存增加 2 辆");
      i18n.global.locale.value = "en-US";
      await w.setProps({ metric: "vehicles" });
      expect(w.find(".tone-workers").exists()).toBe(false);
      expect(w.get(".tone-vehicles").text()).toContain("Inventory +2");
    } finally {
      i18n.global.locale.value = "zh-CN";
      w.unmount();
    }
  });

  it("separates live and static panels and translates custom file controls", async () => {
    vi.mocked(runtimeObserver.listProcesses).mockResolvedValue([]);
    vi.mocked(runtimeObserver.status).mockResolvedValue(idle);
    vi.mocked(runtimeObserver.stop).mockResolvedValue(idle);
    const w = mount(Page, {
      attachTo: document.body,
      global: { plugins: [i18n] },
    });
    await flushPromises();
    expect(w.find(".summary").isVisible()).toBe(true);
    expect(w.find(".diagnostic-controls").isVisible()).toBe(false);
    await w
      .findAll('.mode-tabs [role="tab"]')[1]
      .trigger("mousedown", { button: 0, ctrlKey: false });
    await flushPromises();
    expect(w.find(".summary").isVisible()).toBe(false);
    expect(w.find(".diagnostic-controls").isVisible()).toBe(true);
    expect(runtimeObserver.stop).not.toHaveBeenCalled();
    expect(w.find('input[type="file"]').attributes()).toHaveProperty("hidden");
    i18n.global.locale.value = "en-US";
    await flushPromises();
    expect(w.find(".mode-tabs").text()).toContain("Live log capture");
    expect(w.find(".mode-tabs").text()).toContain("Static log analysis");
    expect(w.find(".file-picker").text()).toContain("Choose file");
    expect(w.find(".file-picker").text()).toContain("No file selected");
    expect(w.find(".file-picker").text()).not.toContain("选择文件");
    i18n.global.locale.value = "zh-CN";
    w.unmount();
  });
  it("renders structured failures and stops without relying on sync", async () => {
    vi.mocked(runtimeObserver.listProcesses).mockResolvedValue([
      { pid: 4, name: "SimCity.exe" },
    ]);
    vi.mocked(runtimeObserver.status).mockResolvedValue({
      ...idle,
      running: true,
      directory: "test",
      status: { ready: true },
    });
    vi.mocked(runtimeObserver.sync).mockRejectedValue({
      code: "io",
      message: "trace unavailable",
    });
    vi.mocked(runtimeObserver.stop).mockResolvedValue(idle);
    const w = mount(Page, { global: { plugins: [i18n] } });
    await flushPromises();
    expect(w.text()).toContain("trace unavailable");
    expect(w.text()).not.toContain("[object Object]");
    await w
      .findAll("button")
      .find((b) => b.text().includes("停止捕获"))!
      .trigger("click");
    await flushPromises();
    expect(runtimeObserver.stop).toHaveBeenCalled();
    w.unmount();
  });
  it("retains stopped logs, filters and scroll position despite empty backend snapshots", async () => {
    vi.useFakeTimers();
    vi.mocked(runtimeObserver.listProcesses).mockResolvedValue([
      { pid: 4, name: "SimCity.exe" },
    ]);
    const row = { sequence: 1, time: 1, name: "saved-for-reading" };
    vi.mocked(runtimeObserver.status).mockResolvedValue({
      ...idle,
      running: true,
      records: 1,
      recent: [row],
    });
    vi.mocked(runtimeObserver.stop).mockResolvedValue(idle);
    vi.mocked(runtimeObserver.start).mockResolvedValue({
      ...idle,
      running: true,
    });
    const w = mount(Page, { global: { plugins: [i18n] } });
    await flushPromises();
    await w.find('input[type="search"]').setValue("saved-for-reading");
    const viewport = w.find(".viewport").element as HTMLElement;
    viewport.scrollTop = 24;
    await w
      .findAll("button")
      .find((b) => b.text().includes("停止捕获"))!
      .trigger("click");
    await flushPromises();
    vi.mocked(runtimeObserver.status).mockResolvedValue(idle);
    await vi.advanceTimersByTimeAsync(3000);
    expect(w.text()).toContain("saved-for-reading");
    expect(viewport.scrollTop).toBe(24);
    expect(
      (w.find('input[type="search"]').element as HTMLInputElement).value,
    ).toBe("saved-for-reading");
    expect(runtimeObserver.status).toHaveBeenCalledTimes(1);
    await w
      .findAll("button")
      .find((b) => b.text().includes("开始捕获"))!
      .trigger("click");
    await flushPromises();
    expect(w.text()).not.toContain("saved-for-reading");
    expect(
      (w.find('input[type="search"]').element as HTMLInputElement).value,
    ).toBe("");
    w.unmount();
  });
  it("selects processes with FDropdown and translates probe errors live", async () => {
    vi.mocked(runtimeObserver.listProcesses).mockResolvedValue([
      { pid: 4, name: "SimCity.exe" },
      { pid: 5, name: "SimCity.exe" },
    ]);
    vi.mocked(runtimeObserver.status).mockResolvedValue(idle);
    vi.mocked(runtimeObserver.start).mockRejectedValue({
      message: JSON.stringify({ code: "binding_missing", message: "fallback" }),
    });
    vi.mocked(runtimeObserver.stop).mockResolvedValue(idle);
    const w = mount(Page, {
      global: { plugins: [i18n], stubs: { teleport: true } },
    });
    await flushPromises();
    expect(w.find("select").exists()).toBe(false);
    await w.find("#observer-process").trigger("click");
    await w.findAll('[role="menuitemradio"]')[1].trigger("click");
    expect(w.find("#observer-process").text()).toContain("PID 5");
    expect(w.find('[role="menu"]').exists()).toBe(false);
    await w
      .findAll("button")
      .find((b) => b.text().includes("开始捕获"))!
      .trigger("click");
    await flushPromises();
    expect(runtimeObserver.start).toHaveBeenCalledWith(
      5,
      60,
      "balanced",
      "ui:all",
    );
    expect(w.find('[role="alert"]').text()).toContain(
      "未找到通过校验的网页绑定",
    );
    i18n.global.locale.value = "en-US";
    await flushPromises();
    expect(w.find('[role="alert"]').text()).toContain(
      "No validated page binding found",
    );
    i18n.global.locale.value = "zh-CN";
    w.unmount();
  });
  it("clears stream history and filters immediately when starting a new capture", async () => {
    vi.mocked(runtimeObserver.listProcesses).mockResolvedValue([
      { pid: 4, name: "SimCity.exe" },
    ]);
    vi.mocked(runtimeObserver.status).mockResolvedValue({
      ...idle,
      recent: [{ sequence: 90, time: 1, name: "old-session" }],
    });
    let resolveStart!: (value: typeof idle) => void;
    vi.mocked(runtimeObserver.start).mockReturnValue(
      new Promise((resolve) => {
        resolveStart = resolve;
      }),
    );
    vi.mocked(runtimeObserver.stop).mockResolvedValue(idle);
    const w = mount(Page, { global: { plugins: [i18n] } });
    await flushPromises();
    expect(w.text()).toContain("old-session");
    await w.find('input[type="search"]').setValue("old");
    await w
      .findAll("button")
      .find((b) => b.text().includes("开始捕获"))!
      .trigger("click");
    expect(w.text()).not.toContain("old-session");
    expect(
      (w.find('input[type="search"]').element as HTMLInputElement).value,
    ).toBe("");
    resolveStart(idle);
    await flushPromises();
    w.unmount();
  });
  it("discards an old status response after a new capture starts", async () => {
    vi.mocked(runtimeObserver.listProcesses).mockResolvedValue([
      { pid: 4, name: "SimCity.exe" },
    ]);
    let resolveOld!: (
      value: Awaited<ReturnType<typeof runtimeObserver.status>>,
    ) => void;
    vi.mocked(runtimeObserver.status).mockReturnValue(
      new Promise((resolve) => {
        resolveOld = resolve;
      }),
    );
    vi.mocked(runtimeObserver.start).mockResolvedValue({
      ...idle,
      running: true,
    });
    vi.mocked(runtimeObserver.stop).mockResolvedValue(idle);
    const w = mount(Page, { global: { plugins: [i18n] } });
    await flushPromises();
    await w
      .findAll("button")
      .find((b) => b.text().includes("开始捕获"))!
      .trigger("click");
    await flushPromises();
    resolveOld({
      ...idle,
      recent: [{ sequence: 90, time: 1, name: "stale-session" }],
      status: { error: "stale-error" },
    });
    await flushPromises();
    expect(w.text()).not.toContain("stale-session");
    expect(w.text()).not.toContain("stale-error");
    w.unmount();
  });
  it("searches payloads and releases rows when cleared", async () => {
    const w = mount(LogStream, {
      props: {
        rows: [
          {
            sequence: 1,
            time: 1,
            name: "PostGameCommand",
            arguments: [{ text: "camera/12" }],
          },
        ],
      },
      global: { plugins: [i18n] },
    });
    expect(w.text()).toContain("camera/12");
    await w.find("input[type=search]").setValue("not-found");
    expect(w.findAll(".line")).toHaveLength(0);
    await w.setProps({ rows: [] });
    await w.find("input[type=search]").setValue("");
    expect(w.findAll(".line")).toHaveLength(0);
    i18n.global.locale.value = "en-US";
    await flushPromises();
    expect(w.text()).toContain("Live log stream");
    i18n.global.locale.value = "zh-CN";
    w.unmount();
  });

  it("shows game time and translates scoped readings when the locale changes", async () => {
    const w = mount(LogStream, {
      props: {
        rows: [
          {
            sequence: 1,
            channel: "asset.resource",
            metric: "workers",
            name: "totalWorkers",
            gameClockSeconds: 59431,
            unitSlot: 7,
            unitStamp: 3,
            before: { amount: 0, capacity: 30 },
            after: { amount: 5, capacity: 30 },
          },
          {
            sequence: 2,
            channel: "asset.resource",
            metric: "tourists",
            name: "touristW1",
            time: 1234567890000,
            unitSlot: 8,
            unitStamp: 4,
            before: null,
            after: { amount: 3, capacity: 20 },
          },
        ],
        metric: "workers",
      },
      global: { plugins: [i18n] },
    });
    try {
      expect(w.find("time").text()).toBe("16:30:31");
      expect(w.text()).toContain("在岗劳工");
      expect(w.text()).toContain("0 / 30 → 5 / 30");
      expect(w.findAll(".line")).toHaveLength(1);
      i18n.global.locale.value = "en-US";
      await w.setProps({ metric: "tourists" });
      expect(w.find("time").text()).toBe("—");
      expect(w.text()).toContain("W1 tourists");
      expect(w.text()).toContain("No bin → 3 / 20");
    } finally {
      i18n.global.locale.value = "zh-CN";
      w.unmount();
    }
  });
});
