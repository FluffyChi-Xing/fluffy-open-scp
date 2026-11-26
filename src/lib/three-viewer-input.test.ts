import { afterEach, describe, expect, it, vi } from "vitest";
import { ThreeViewer } from "./three-viewer";
import { createInteractionFrameBudget } from "./interactionFrameBudget";
import { createFramePixelRatio } from "./framePixelRatio";
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

function dragHarness() {
  const prototype = ThreeViewer.prototype;
  const beginCameraInteraction = vi.fn();
  const orbit = vi.fn();
  const viewer = Object.assign(Object.create(prototype), {
    activePointerId: 1,
    lastX: 0,
    lastY: 0,
    moved: 0,
    pendingDragX: 0,
    pendingDragY: 0,
    pointerButton: 0,
    pointerShift: false,
    container: { hasPointerCapture: () => false },
    options: { onTap: vi.fn() },
    orbit,
    beginCameraInteraction,
    pick: vi.fn(),
  });
  // Arrow handlers belong to the instance. Test delta consumption and
  // cancellation through their shared methods without constructing WebGL.
  const queue = (x: number) => Reflect.get(prototype, "queueDrag").call(viewer, { clientX: x, clientY: 0 });
  const flush = () => Reflect.get(prototype, "flushDrag").call(viewer);
  const cancel = () => Reflect.get(prototype, "cancelCameraDrag").call(viewer);
  return { queue, flush, cancel, orbit, beginCameraInteraction };
}

describe("ThreeViewer camera input", () => {
  it("restores full pixel ratio and shadow updates after reduced-quality motion", () => {
    vi.useFakeTimers();
    vi.stubGlobal("devicePixelRatio", 2);
    let ratio = 1.5;
    const renderer = {
      getPixelRatio: () => ratio,
      setPixelRatio: (value: number) => { ratio = value; },
      shadowMap: { enabled: true, autoUpdate: true, needsUpdate: false },
    };
    const viewer = Object.assign(Object.create(ThreeViewer.prototype), {
      options: { maxPixelRatio: 1.5, interactionPixelRatio: 1 },
      renderer, keyLight: { castShadow: true }, interactionActive: false,
      activePointerId: null, qualityRestoreTimer: null, interactionShadowState: false,
      interactionBudget: createInteractionFrameBudget(), framePixelRatio: createFramePixelRatio(), invalidate: vi.fn(),
    });
    Reflect.get(ThreeViewer.prototype, "beginCameraInteraction").call(viewer);
    expect(ratio).toBe(1.5); // keep the last complete image until the next draw
    viewer.framePixelRatio.draw(renderer, () => {});
    expect(ratio).toBe(1);
    expect(renderer.shadowMap.autoUpdate).toBe(false);
    renderer.setPixelRatio(0.6);
    Reflect.get(ThreeViewer.prototype, "scheduleQualityRestore").call(viewer);
    vi.advanceTimersByTime(180);
    expect(ratio).toBe(0.6);
    viewer.framePixelRatio.draw(renderer, () => {});
    expect(ratio).toBe(1.5);
    expect(renderer.shadowMap.autoUpdate).toBe(true);
    expect(renderer.shadowMap.needsUpdate).toBe(true);
    expect(viewer.interactionActive).toBe(false);
  });
  it("combines a burst into one transform and retains the final movement", () => {
    const { queue, flush, orbit, beginCameraInteraction } = dragHarness();
    queue(1);
    flush();
    expect(orbit).not.toHaveBeenCalled();
    queue(4);
    queue(12);
    expect(orbit).not.toHaveBeenCalled();
    flush();
    flush();
    expect(orbit.mock.calls).toEqual([[12, 0]]);
    expect(beginCameraInteraction).toHaveBeenCalledTimes(1);
    queue(13);
    flush();
    expect(orbit.mock.calls).toEqual([[12, 0], [1, 0]]);
  });

  it("does not apply an outstanding delta after capture is cancelled", () => {
    const { queue, flush, cancel, orbit } = dragHarness();
    queue(40);
    cancel();
    flush();
    expect(orbit).not.toHaveBeenCalled();
  });

  it("reverses immediately after dragging beyond an orbit pole", () => {
    const viewer = { orbitTheta: 0, orbitPhi: 1.2, applyCamera: vi.fn() };
    const orbit = Reflect.get(ThreeViewer.prototype, "orbit");
    orbit.call(viewer, 0, 400);
    expect(viewer.orbitPhi).toBeCloseTo(0.02);
    orbit.call(viewer, 0, -2);
    expect(viewer.orbitPhi).toBeGreaterThan(0.02);
    expect(viewer.orbitPhi).toBeLessThan(0.05);
  });
});

it.each([false, true])("clears only retained particle frames on scene rebuild (particles: %s)", (lastFrameHadParticles) => {
  const clear = vi.fn();
  const viewer = Object.assign(Object.create(ThreeViewer.prototype), {
    lastFrameHadParticles,
    renderer: { clear },
    deferredDisposals: { setPaused: vi.fn() },
  });
  viewer.setSceneAssembling(true);
  viewer.setSceneAssembling(true);
  expect(clear).toHaveBeenCalledTimes(lastFrameHadParticles ? 1 : 0);
});
