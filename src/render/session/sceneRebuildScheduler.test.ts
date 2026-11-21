import { expect, it, vi } from "vitest";
import { createSceneRebuildScheduler } from "./sceneRebuildScheduler";
it("coalesces frame requests, serializes in-flight work and cancels on disposal", async () => {
  const frames: (() => void)[] = [];
  let finish!: () => void;
  const rebuild = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise<void>((r) => {
          finish = r;
        }),
    )
    .mockResolvedValue(undefined);
  const scheduler = createSceneRebuildScheduler({
    suspended: () => false,
    whenSettled: async () => {},
    rebuild,
    requestFrame: (cb) => {
      frames.push(cb);
    },
    onError: (error) => {
      throw error;
    },
  });
  scheduler.request();
  scheduler.request();
  expect(frames).toHaveLength(1);
  frames.shift()!();
  await Promise.resolve();
  await Promise.resolve();
  scheduler.request();
  expect(rebuild).toHaveBeenCalledTimes(1);
  finish();
  await vi.waitFor(() => expect(rebuild).toHaveBeenCalledTimes(2));
  scheduler.request();
  scheduler.dispose();
  frames.shift()?.();
  await Promise.resolve();
  expect(rebuild).toHaveBeenCalledTimes(2);
});
