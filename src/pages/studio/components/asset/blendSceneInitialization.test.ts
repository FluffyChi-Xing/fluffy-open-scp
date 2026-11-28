import { describe, expect, it, vi } from "vitest";
import { initializeBlendScene } from "./blendSceneInitialization";

describe("Blend scene initialization failure boundary", () => {
  it("continues independent stages and reports their original failures", async () => {
    const failure = new Error("scene failed");
    const cache = vi.fn(async () => {});
    const failures = await initializeBlendScene([
      {
        id: "scene",
        run: async () => {
          throw failure;
        },
      },
      { id: "cache", run: cache },
    ]);
    expect(cache).toHaveBeenCalledOnce();
    expect(failures).toEqual([{ step: "scene", cause: failure }]);
  });

  it("executes stages in order without reporting success as a failure", async () => {
    const order: number[] = [];
    expect(
      await initializeBlendScene([
        {
          id: "register",
          run: async () => {
            order.push(1);
          },
        },
        {
          id: "save",
          run: async () => {
            order.push(2);
          },
        },
      ]),
    ).toEqual([]);
    expect(order).toEqual([1, 2]);
  });
});
