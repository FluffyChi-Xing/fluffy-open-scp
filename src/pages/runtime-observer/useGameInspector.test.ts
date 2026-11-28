import { defineComponent, shallowRef } from "vue";
import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { command, subscribe } from "@/api/tauri";
import { useGameInspector } from "./useGameInspector";
vi.mock("@/api/tauri", () => ({
  command: vi.fn().mockResolvedValue(undefined),
  subscribe: vi.fn(),
}));
it("accepts only current-session and current-frame selections and releases its connection", async () => {
  let event!: (value: unknown) => void;
  const stop = vi.fn();
  vi.mocked(subscribe).mockImplementation(async (_name, cb) => {
    event = cb;
    return stop;
  });
  const select = vi.fn(),
    snapshot = vi.fn().mockResolvedValue(undefined);
  let inspector!: ReturnType<typeof useGameInspector>;
  const w = mount(
    defineComponent({
      setup() {
        inspector = useGameInspector(
          () => 42,
          shallowRef({
            pid: 42,
            time: 100,
            sequence: 3,
            viewport: [100, 100],
            boxes: [],
          }),
          snapshot,
          select,
        );
        return () => null;
      },
    }),
  );
  await inspector.connect();
  const call = vi
    .mocked(command)
    .mock.calls.find(([name]) => name === "devtool_connect")!;
  const session = (call[1] as { session: string }).session;
  event({
    kind: "selected",
    pid: 42,
    session: "previous",
    node: 8,
    sequence: 3,
    time: 100,
  });
  event({
    kind: "selected",
    pid: 42,
    session,
    node: 8,
    sequence: 2,
    time: 100,
  });
  expect(select).not.toHaveBeenCalled();
  event({ kind: "ready", pid: 42, session });
  expect(inspector.connected.value).toBe(true);
  event({
    kind: "selected",
    pid: 42,
    session,
    node: 8,
    sequence: 3,
    time: 100,
  });
  expect(select).toHaveBeenCalledWith(8);
  event({ kind: "snapshot", pid: 42, session });
  expect(snapshot).toHaveBeenCalledTimes(1);
  event({kind:"closed",pid:42,session});
  expect(inspector.connected.value).toBe(false);
  w.unmount();
  await flushPromises();
  expect(stop).toHaveBeenCalled();
  expect(command).toHaveBeenCalledWith("devtool_disconnect", { pid: 42 });
});
