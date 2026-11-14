import { describe, expect, it } from "vitest";
import { viewportToPixel } from "./viewport";

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
});
