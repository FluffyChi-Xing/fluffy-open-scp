import { mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const state = vi.hoisted(() => ({
  tauri: true,
  summary: vi.fn(),
  clear: vi.fn(),
}));

vi.mock("@/api", () => ({
  isTauri: () => state.tauri,
  tauriApi: { renderTelemetry: { summary: state.summary, clear: state.clear } },
}));

const { useRenderTelemetry } = await import("./useRenderTelemetry");

type Telemetry = ReturnType<typeof useRenderTelemetry>;
let telemetry: Telemetry;

const Harness = defineComponent({
  setup() {
    telemetry = useRenderTelemetry(7);
    return () => null;
  },
});

describe("useRenderTelemetry", () => {
  beforeEach(() => {
    state.tauri = true;
    state.summary.mockReset().mockResolvedValue({
      windowDays: 7,
      since: 0,
      totalCount: 0,
      stages: [],
    });
    state.clear.mockReset().mockResolvedValue({ rows: 2 });
  });

  it("挂载即拉取一次统计", async () => {
    mount(Harness);
    await Promise.resolve();
    await Promise.resolve();
    expect(state.summary).toHaveBeenCalledWith(7);
    expect(telemetry.summary.value).not.toBeNull();
  });

  it("clear 先清库、再重新拉取（顺序不可颠倒）", async () => {
    mount(Harness);
    await Promise.resolve();
    await Promise.resolve();
    const before = state.summary.mock.calls.length;

    await telemetry.clear();

    expect(state.clear).toHaveBeenCalledTimes(1);
    expect(state.summary.mock.calls.length).toBe(before + 1);
    const clearOrder = state.clear.mock.invocationCallOrder[0];
    const reloadOrder = state.summary.mock.invocationCallOrder[before];
    expect(clearOrder).toBeLessThan(reloadOrder);
  });

  it("非 Tauri 环境不调用后端", async () => {
    state.tauri = false;
    mount(Harness);
    await Promise.resolve();
    expect(state.summary).not.toHaveBeenCalled();
    await telemetry.clear();
    expect(state.clear).not.toHaveBeenCalled();
  });
});
