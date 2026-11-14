import { describe, expect, it } from "vitest";
import {
  groupPathPointPolylines,
  parsePathPairRanges,
  type PathPointLike,
} from "./path";

function pt(
  index: number,
  point: [number, number, number] = [index, 0, 0],
  pointIndex: number | null = index,
): PathPointLike {
  return { index, point, pointIndex };
}

describe("parsePathPairRanges", () => {
  it("parses inclusive (start,end) pairs", () => {
    expect(parsePathPairRanges([0, 2, 3, 5])).toEqual([
      { start: 0, end: 2 },
      { start: 3, end: 5 },
    ]);
  });

  it("drops odd trailing and inverted ranges", () => {
    expect(parsePathPairRanges([0, 1, 9])).toEqual([{ start: 0, end: 1 }]);
    expect(parsePathPairRanges([5, 1, 2, 4])).toEqual([{ start: 2, end: 4 }]);
  });
});

describe("groupPathPointPolylines", () => {
  it("splits six points into two parallel paths by pairs", () => {
    // 用户截图语义：两组各 3 点沿同向并行，pairs = [0,2, 3,5]
    const points = [0, 1, 2, 3, 4, 5].map((i) =>
      pt(i, [i < 3 ? 0 : 2, 0, i % 3]),
    );
    const groups = groupPathPointPolylines(points, [0, 2, 3, 5]);
    expect(groups).toHaveLength(2);
    expect(groups[0]!.map((p) => p.index)).toEqual([0, 1, 2]);
    expect(groups[1]!.map((p) => p.index)).toEqual([3, 4, 5]);
  });

  it("matches fire-prop three short paths", () => {
    const points = [0, 1, 2, 3, 4, 5].map((i) => pt(i));
    const groups = groupPathPointPolylines(points, [0, 1, 2, 3, 4, 5]);
    expect(groups.map((g) => g.map((p) => p.index))).toEqual([
      [0, 1],
      [2, 3],
      [4, 5],
    ]);
  });

  it("matches balloon [0,1]+[2,4]", () => {
    const points = [0, 1, 2, 3, 4].map((i) => pt(i));
    const groups = groupPathPointPolylines(points, [0, 1, 2, 4]);
    expect(groups.map((g) => g.map((p) => p.index))).toEqual([
      [0, 1],
      [2, 3, 4],
    ]);
  });

  it("skips single-point ranges and ignores missing indices", () => {
    const points = [pt(0), pt(2), pt(3)];
    const groups = groupPathPointPolylines(points, [0, 0, 2, 3]);
    expect(groups.map((g) => g.map((p) => p.index))).toEqual([[2, 3]]);
  });

  it("falls back to one polyline sorted by pointIndex when pairs empty", () => {
    const points = [pt(2, [2, 0, 0], 20), pt(0, [0, 0, 0], 0), pt(1, [1, 0, 0], 10)];
    const groups = groupPathPointPolylines(points, []);
    expect(groups).toHaveLength(1);
    expect(groups[0]!.map((p) => p.index)).toEqual([0, 1, 2]);
  });

  it("ignores points without coordinates", () => {
    const points = [pt(0), { index: 1, point: null, pointIndex: 1 }, pt(2)];
    const groups = groupPathPointPolylines(points, [0, 2]);
    expect(groups[0]!.map((p) => p.index)).toEqual([0, 2]);
  });
});
