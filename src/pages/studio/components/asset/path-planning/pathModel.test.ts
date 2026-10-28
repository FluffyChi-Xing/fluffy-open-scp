import { describe, expect, it } from "vitest";
import type { LotUnitDto } from "@/api/tauri";
import {
  adoptOrphanPaths,
  nextPathId,
  pathAutoTangent,
  pathPointPositions,
  recomputePathTangents,
  validateWorkbenchPath,
  type WorkbenchPath,
} from "./pathModel";

const point = (x: number, y: number, z = 0): [number, number, number] => [x, y, z];

const base: WorkbenchPath = {
  id: "path:1",
  label: "前门",
  entry: "418A18B1",
  scope: "local",
  roadConnect: false,
  pointIds: ["pathPoint:0", "pathPoint:1"],
};

const pathPointUnit = (
  index: number,
  point: [number, number, number] | null,
  tangent: [number, number, number] | null = null,
): LotUnitDto =>
  ({
    kind: "pathPoint",
    index,
    point,
    tangent,
    pointIndex: index,
    fields: [],
  }) as unknown as LotUnitDto;

describe("pathAutoTangent", () => {
  it("interior points take half the neighbor chord (Catmull-Rom midpoint)", () => {
    expect(pathAutoTangent(point(0, 0), point(4, 0), point(0, 8))).toEqual([
      0, 4, 0,
    ]);
  });
  it("first point takes the full outgoing chord", () => {
    expect(pathAutoTangent(null, point(0, 0), point(6, 2, 4))).toEqual([
      6, 2, 4,
    ]);
  });
  it("last point takes the full incoming chord", () => {
    expect(pathAutoTangent(point(2, 0, 0), point(0, 0, 0), null)).toEqual([
      -2, 0, 0,
    ]);
  });
  it("isolated point degenerates to zero tangent", () => {
    expect(pathAutoTangent(null, point(1, 2, 3), null)).toEqual([0, 0, 0]);
  });
});

describe("recomputePathTangents", () => {
  it("keeps output length equal to input (engine requires equal arrays)", () => {
    const points = [point(0, 0), point(2, 0), point(2, 4), null];
    const tangents = recomputePathTangents(points);
    expect(tangents).toHaveLength(4);
    expect(tangents[3]).toBeNull();
    expect(tangents[0]).toEqual([2, 0, 0]);
    expect(tangents[1]).toEqual([1, 2, 0]);
  });
});

describe("nextPathId", () => {
  it("continues the numeric sequence", () => {
    expect(nextPathId([])).toBe("path:1");
    expect(nextPathId(["path:1", "path:7"])).toBe("path:8");
  });
});

describe("validateWorkbenchPath", () => {
  it("passes a bound path with two tangent-bearing points", () => {
    const points = [
      pathPointUnit(0, point(0, 0), point(1, 0, 0)),
      pathPointUnit(1, point(4, 0), point(1, 0, 0)),
    ];
    expect(validateWorkbenchPath(base, points)).toEqual([]);
  });
  it("flags missing entry as engine-level issue", () => {
    const issues = validateWorkbenchPath(
      { ...base, entry: null },
      [pathPointUnit(0, point(0, 0), point(1, 0, 0)), pathPointUnit(1, point(4, 0), point(1, 0, 0))],
    );
    expect(issues).toContain("entry-missing");
  });
  it("flags too-few points and missing tangents", () => {
    const issues = validateWorkbenchPath(base, [
      pathPointUnit(0, point(0, 0), null),
      null,
    ]);
    expect(issues).toContain("points-too-few");
    expect(issues).toContain("tangent-missing");
    expect(issues).toContain("point-missing");
  });
});

describe("pathPointPositions", () => {
  it("maps ids to coordinates and keeps gaps as null", () => {
    const path: WorkbenchPath = {
      ...base,
      pointIds: ["pathPoint:0", "pathPoint:9"],
    };
    const byId = new Map<string, LotUnitDto>([
      ["pathPoint:0", pathPointUnit(0, point(1, 2, 3), null)],
    ]);
    expect(pathPointPositions(path, byId)).toEqual([
      point(1, 2, 3),
      null,
    ]);
  });
});

describe("adoptOrphanPaths", () => {
  it("adopts unreferenced pathPoints sorted by pointIndex", () => {
    const units = [
      pathPointUnit(3, point(3, 0), null),
      pathPointUnit(1, point(1, 0), null),
    ];
    const result = adoptOrphanPaths([], units);
    expect(result).not.toBeNull();
    expect(result![0].pointIds).toEqual(["pathPoint:1", "pathPoint:3"]);
    expect(result![0].entry).toBeNull();
  });
  it("skips points already referenced by an existing path", () => {
    const path: WorkbenchPath = { ...base, pointIds: ["pathPoint:0"] };
    const units = [pathPointUnit(0, point(0, 0)), pathPointUnit(2, point(2, 0))];
    const result = adoptOrphanPaths([path], units);
    expect(result![0].pointIds).toEqual(["pathPoint:0"]);
    expect(result![1].pointIds).toEqual(["pathPoint:2"]);
    expect(result![1].id).toBe("path:2");
  });
  it("returns null when no orphans exist (idempotent)", () => {
    const path: WorkbenchPath = { ...base, pointIds: ["pathPoint:0"] };
    expect(adoptOrphanPaths([path], [pathPointUnit(0, point(0, 0))])).toBeNull();
  });
});
