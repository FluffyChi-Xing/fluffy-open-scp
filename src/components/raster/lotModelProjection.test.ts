import { describe, expect, it, vi } from "vitest";
import { BufferGeometry, Float32BufferAttribute, Mesh } from "three";
import { drawLotProjection } from "./lotModelProjection";

describe("lot model projection", () => {
  it("projects transformed geometry into lot coordinates from the south-up mask origin", () => {
    const geometry = new BufferGeometry().setAttribute("position", new Float32BufferAttribute([
      0, 0, 20, 2, 0, 20, 0, 4, 20,
    ], 3));
    geometry.setIndex([0, 2, 1]);
    const mesh = new Mesh(geometry);
    mesh.position.set(1, 1, 0);
    const context = {
      clearRect: vi.fn(), beginPath: vi.fn(), moveTo: vi.fn(),
      lineTo: vi.fn(), closePath: vi.fn(), fill: vi.fn(),
    };
    drawLotProjection(context as unknown as CanvasRenderingContext2D, [mesh], {
      origin: [1, -1, 0],
      transform: [0, 2, 0, -2, 0, 0, 0, 0, 2, 2, 1, 0],
      center: [1, 2],
    }, [20, 40], 20, 40);
    expect(context.moveTo.mock.calls[0]).toEqual([11, 23]);
    expect(context.lineTo.mock.calls).toEqual([[11, 27], [3, 23]]);
    expect(context.fill).toHaveBeenCalledOnce();
  });
});
