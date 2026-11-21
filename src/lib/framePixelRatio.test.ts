import { expect, it, vi } from "vitest";
import { createFramePixelRatio } from "./framePixelRatio";

it("never leaves a completed frame cleared between adaptive updates", () => {
  const frame = createFramePixelRatio();
  let ratio = 1, pixels = "scene";
  const host = {
    getPixelRatio: () => ratio,
    setPixelRatio: (value: number) => { ratio = value; pixels = "blank"; },
  };
  for (const next of [0.8, 0.5, 1.5]) {
    frame.request(next);
    expect(pixels).toBe("scene");
    frame.draw(host, () => { expect(pixels).toBe("blank"); pixels = "scene"; });
    expect(ratio).toBe(next);
    expect(pixels).toBe("scene");
  }
});

it("lets a new drag supersede a queued restoration without clearing at the same ratio", () => {
  const frame = createFramePixelRatio();
  const host = { getPixelRatio: () => 1, setPixelRatio: vi.fn() };
  frame.request(1.5);
  expect(frame.desired(1)).toBe(1.5);
  frame.request(1);
  const draw = vi.fn();
  frame.draw(host, draw);
  expect(host.setPixelRatio).not.toHaveBeenCalled();
  expect(draw).toHaveBeenCalledOnce();
});
