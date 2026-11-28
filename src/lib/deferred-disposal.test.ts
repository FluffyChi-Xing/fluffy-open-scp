import { afterEach, expect, it, vi } from "vitest";
import { Group } from "three";
import { DeferredDisposalQueue } from "./deferred-disposal";

afterEach(() => { vi.useRealTimers(); });

it("retains old materials across assembly yields and releases after replacement preparation", () => {
  vi.useFakeTimers();
  const dispose = vi.fn();
  const queue = new DeferredDisposalQueue(dispose);
  const old = new Group();
  queue.enqueue(old);
  queue.setPaused(true);
  vi.runAllTimers();
  expect(dispose).not.toHaveBeenCalled();
  queue.setPaused(false);
  vi.runAllTimers();
  expect(dispose).toHaveBeenCalledExactlyOnceWith(old);
});

it("drains paused resources when the viewer is destroyed", () => {
  const dispose = vi.fn();
  const queue = new DeferredDisposalQueue(dispose);
  queue.setPaused(true);
  queue.enqueue(new Group());
  queue.drain();
  expect(dispose).toHaveBeenCalledOnce();
});
