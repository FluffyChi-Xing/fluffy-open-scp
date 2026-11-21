import { expect, it, vi } from "vitest";
import { ThreeViewer } from "./three-viewer";
it("prepares programs asynchronously and does not finish before driver completion", async () => {
  let finish!: () => void;
  const renderer = {
    compile: vi.fn(),
    compileAsync: vi.fn(() => new Promise<void>(resolve => { finish = resolve; })),
  };
  const host = { disposed: false, renderer, scene: {}, camera: {} };
  let done = false;
  const pending = ThreeViewer.prototype.prepareShaders.call(host as unknown as ThreeViewer).then(() => { done = true; });
  expect(renderer.compileAsync).toHaveBeenCalledOnce();
  await Promise.resolve();
  expect(done).toBe(false);
  expect(renderer.compile).not.toHaveBeenCalled();
  finish();
  await pending;
  expect(done).toBe(true);
});
