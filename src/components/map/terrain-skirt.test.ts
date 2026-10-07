import { describe, expect, it } from "vitest";
import { createTerrainSkirtGeometry } from "./terrain-skirt";

describe("terrain cut faces", () => {
  for (const verts of [17, 65, 257]) {
    it(`matches every active LOD edge exactly (${verts} vertices)`, () => {
      const source = new Float32Array(verts * verts * 3);
      for (let y = 0; y < verts; y++)
        for (let x = 0; x < verts; x++) {
          source.set(
            [x * 8, Math.sin(x * 0.7) * 100 + Math.cos(y) * 70, y * 8],
            (y * verts + x) * 3,
          );
        }
      const geo = createTerrainSkirtGeometry(
        source,
        verts,
        { north: true, east: true, south: true, west: true },
        -1024,
      );
      const p = geo.getAttribute("position"),
        n = geo.getAttribute("normal");
      const expectedIds = [
        ...Array.from({ length: verts }, (_, i) => i),
        ...Array.from({ length: verts }, (_, i) => i * verts + verts - 1),
        ...Array.from({ length: verts }, (_, i) => verts * verts - 1 - i),
        ...Array.from({ length: verts }, (_, i) => (verts - 1 - i) * verts),
      ];
      expectedIds.forEach((id, i) => {
        expect([p.getX(i * 2), p.getY(i * 2), p.getZ(i * 2)]).toEqual(
          Array.from(source.slice(id * 3, id * 3 + 3)),
        );
        expect(p.getX(i * 2 + 1)).toBe(p.getX(i * 2));
        expect(p.getZ(i * 2 + 1)).toBe(p.getZ(i * 2));
        expect(p.getY(i * 2 + 1)).toBe(-1024);
        expect(n.getY(i * 2)).toBe(0);
      });
      expect([n.getX(0), n.getZ(0)]).toEqual([0, -1]);
      expect([n.getX(verts * 2), n.getZ(verts * 2)]).toEqual([1, 0]);
      geo.dispose();
    });
  }
  it("does not create cut faces on internal chunks", () => {
    const geo = createTerrainSkirtGeometry(
      new Float32Array(27),
      3,
      { north: false, east: false, south: false, west: false },
      -1024,
    );
    expect(geo.getAttribute("position").count).toBe(0);
    geo.dispose();
  });
});
