import { describe, expect, it } from "vitest";
import { viewportToPixel } from "./viewport";
import { oppositeMaskRow } from "@/lib/lot-ground/maskRows";
import { PerspectiveCamera, Vector3 } from "three";

describe("viewportToPixel", () => {
  it("accounts for the centered/panned canvas before scaling", () => {
    expect(
      viewportToPixel(
        { x: 150, y: 90 },
        { left: 10, top: 20 },
        { x: 40, y: 10 },
        2,
      ),
    ).toEqual({ x: 50, y: 30 });
  });

  it("supports negative pan while preserving pixel floor semantics", () => {
    expect(
      viewportToPixel(
        { x: 9, y: 19 },
        { left: 10, top: 20 },
        { x: -4, y: -6 },
        1.5,
      ),
    ).toEqual({ x: 2, y: 3 });
  });

  it("uses the visible depth scale when editing a rectangular lot", () => {
    expect(viewportToPixel(
      { x: 60, y: 220 }, { left: 10, top: 20 }, { x: 0, y: 0 }, 1, 2.5,
    )).toEqual({ x: 50, y: 80 });
  });

  it("maps the canvas upper-left to the rendered Lot's upper-left", () => {
    const width = 4, height = 3;
    const camera = new PerspectiveCamera(45, 1, 0.1, 100);
    camera.position.set(0.35, 0.4, 1).normalize().multiplyScalar(12);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld();
    const projected = (x: number, y: number) => {
      const row = oppositeMaskRow(y, height);
      // PlaneGeometry: U increases towards +X and V towards +Y. The
      // south-first mask uses flipY=false, and the Z-up world rotates -90°.
      return new Vector3(x + 0.5 - width / 2, 0, height / 2 - row - 0.5).project(camera);
    };
    const upperLeft = projected(0, 0);
    expect(upperLeft.x).toBeLessThan(projected(3, 0).x);
    expect(upperLeft.y).toBeGreaterThan(projected(0, 2).y);
  });
});
