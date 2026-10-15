import { describe, expect, it } from "vitest";
import type { RoadSweep } from "@/lib/region-map";
import { sweepSections } from "./map-sweep";

const component = {
  interval: 0,
  start: 0,
  end: 0,
  length: 0,
  step: 0,
  distort: true,
  repeatUv: false,
  roundIntervals: false,
  instance: false,
} as RoadSweep;
describe("original road sweep semantics", () => {
  it("places thin closure meshes only at the two ends of the bridge", () => {
    const a = sweepSections(
      { ...component, length: 0.1, roundIntervals: true },
      1200,
      0.5434,
    );
    const b = sweepSections(
      { ...component, length: -0.1, roundIntervals: true },
      1200,
      0.5434,
    );
    expect(a).toHaveLength(1);
    expect(b).toHaveLength(1);
    expect(a[0].start).toBe(0);
    expect(b[0].start).toBeCloseTo(1199.9, 3);
  });
  it("scales cable spans and tower spacing by the same rounded interval", () => {
    const cables = sweepSections(
      { ...component, interval: 500, step: 500, roundIntervals: true },
      1200,
      458,
    );
    const towers = sweepSections(
      {
        ...component,
        interval: 500,
        start: 250,
        roundIntervals: true,
        instance: true,
      },
      1200,
      5,
    );
    expect(cables).toHaveLength(2);
    expect(towers).toHaveLength(2);
    expect(cables[0].length).toBeCloseTo(600, 3);
    expect(towers[0].start).toBeCloseTo(300, 3);
    expect(towers[1].start).toBeCloseTo(900, 3);
  });
});
