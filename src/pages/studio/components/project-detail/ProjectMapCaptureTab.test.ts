import { flushPromises, mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import { command } from "@/api/tauri";
import ProjectMapCaptureTab from "./ProjectMapCaptureTab.vue";

vi.mock("@/api/runtimeObserver", () => ({
  runtimeObserver: { listProcesses: vi.fn().mockResolvedValue([{ pid: 17, name: "SimCity.exe" }]) },
}));
vi.mock("@/api/tauri", async (load) => ({ ...(await load<object>()), command: vi.fn() }));
vi.mock("@/api", () => ({ tauriApi: { studio: { projects: { repoSummary: vi.fn() } } } }));

it("starts the map probe from the project without requiring a building build", async () => {
  vi.mocked(command).mockResolvedValue({
    directory: "map-test", running: true, status: { ready: true, tracked: 6 },
    recent: [{ sequence: 1, channel: "map.resource", name: "coal", changedCells: 1,
      increased: 0, decreased: 32, amountDelta: -32 }],
  });
  i18n.global.locale.value = "zh-CN";
  const wrapper = mount(ProjectMapCaptureTab, {
    props: { project: "map-test" }, global: { plugins: [i18n] },
  });
  try {
    await flushPromises();
    expect(wrapper.text()).toContain("资源图层");
    expect(wrapper.text()).not.toContain("最近构建");
    await wrapper.findAll("button").find((button) => button.text() === "开始捕获")!.trigger("click");
    await flushPromises();
    expect(command).toHaveBeenCalledWith("observer_start", {
      pid: 17, seconds: 300, observation: "map:resources", projectCapture: true,
    });
    expect(wrapper.text()).toContain("减少 32");
  } finally {
    wrapper.unmount();
    await flushPromises();
  }
});
