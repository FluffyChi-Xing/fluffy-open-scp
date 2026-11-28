import { mount, flushPromises } from "@vue/test-utils";
import { afterEach, expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import ElementsPanel from "./ElementsPanel.vue";
import { readElements } from "./elements";
import { runtimeObserver } from "@/api/runtimeObserver";
vi.mock("./elements", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./elements")>()),
  readElements: vi.fn(),
}));
vi.mock("@/api/runtimeObserver", () => ({
  runtimeObserver: { frame: vi.fn() },
}));
afterEach(() => vi.clearAllMocks());
const shot = { mime: "image/png", dataBase64: "", capturedAt: 0 };
const snapshot = {
  pid: 1,
  time: 1,
  sequence: 1,
  viewport: [100, 100],
  boxes: [{ i: 0, parent: -1, tag: "html" }],
};
it("does not poll and retains the DOM when screen capture fails", async () => {
  vi.mocked(readElements).mockResolvedValue(snapshot);
  vi.mocked(runtimeObserver.frame).mockRejectedValue(
    new Error("capture unavailable"),
  );
  const w = mount(ElementsPanel, {
    props: { pid: 1 },
    global: { plugins: [i18n] },
  });
  expect(readElements).not.toHaveBeenCalled();
  await w.findAll("button").at(-1)!.trigger("click");
  await flushPromises();
  expect(w.findAll(".element-row")).toHaveLength(1);
  expect(w.get('[role="alert"]').text()).toContain("capture unavailable");
  expect(readElements).toHaveBeenCalledTimes(1);
  w.unmount();
});
it("ignores a stale process response after changing the selected process", async () => {
  let resolve!: (value: typeof snapshot) => void;
  vi.mocked(readElements).mockReturnValue(
    new Promise((r) => {
      resolve = r;
    }),
  );
  vi.mocked(runtimeObserver.frame).mockResolvedValue(shot);
  const w = mount(ElementsPanel, {
    props: { pid: 1 },
    global: { plugins: [i18n] },
  });
  await w.findAll("button").at(-1)!.trigger("click");
  await w.setProps({ pid: 2 });
  resolve(snapshot);
  await flushPromises();
  expect(w.findAll(".element-row")).toHaveLength(0);
  expect(runtimeObserver.frame).not.toHaveBeenCalled();
  w.unmount();
});
