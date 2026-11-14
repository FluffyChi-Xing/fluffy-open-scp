import { describe, expect, it, vi } from "vitest";
import { ThreeViewer } from "./three-viewer";

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
