import { expect, it } from "vitest";
import { layoutMultilineLettering } from "./multilineLettering";

it("fits the longest line and centers the whole block with consistent spacing", () => {
  const measure = (line: string, size: number) => line.length * size;
  const layout = layoutMultilineLettering("HOT\r\n\r\nMARKET", 120, 90, measure)!;
  expect(layout.lines.map((line) => line.text)).toEqual(["HOT", "", "MARKET"]);
  expect(layout.fontSize * 6).toBeCloseTo(120 * 0.86);
  const [top, middle, bottom] = layout.lines.map(({ y }) => y);
  expect(middle).toBe(45);
  expect(middle - top).toBeCloseTo(bottom - middle);
  expect(layoutMultilineLettering("HOT\n", 120, 90, measure)).toBeNull();
});
