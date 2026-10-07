import * as THREE from "three";

/** A separate vertical face with an exact copy of the active terrain LOD edge.
 * Each edge owns its vertices/normals so corners and the top remain hard.
 */
export function createTerrainSkirtGeometry(
  terrainPositions: Float32Array,
  verts: number,
  borders: { north: boolean; east: boolean; south: boolean; west: boolean },
  bottom: number,
): THREE.BufferGeometry {
  const pos: number[] = [],
    normals: number[] = [],
    colors: number[] = [],
    indices: number[] = [];
  const topColor = new THREE.Color("#897958");
  const bottomColor = new THREE.Color("#42454a");
  const edge = (ids: number[], nx: number, nz: number) => {
    const base = pos.length / 3;
    for (const id of ids) {
      const k = id * 3;
      const x = terrainPositions[k],
        y = terrainPositions[k + 1],
        z = terrainPositions[k + 2];
      pos.push(x, y, z, x, Math.min(bottom, y), z);
      normals.push(nx, 0, nz, nx, 0, nz);
      colors.push(
        topColor.r,
        topColor.g,
        topColor.b,
        bottomColor.r,
        bottomColor.g,
        bottomColor.b,
      );
    }
    for (let i = 0; i < ids.length - 1; i++) {
      const a = base + i * 2;
      indices.push(a, a + 2, a + 1, a + 2, a + 3, a + 1);
    }
  };
  const row = Array.from({ length: verts }, (_, i) => i);
  if (borders.north) edge(row, 0, -1);
  if (borders.east)
    edge(
      row.map((i) => i * verts + verts - 1),
      1,
      0,
    );
  if (borders.south)
    edge(
      row.map((i) => verts * verts - 1 - i),
      0,
      1,
    );
  if (borders.west)
    edge(
      row.map((i) => (verts - 1 - i) * verts),
      -1,
      0,
    );
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  geometry.setAttribute("normal", new THREE.Float32BufferAttribute(normals, 3));
  geometry.setAttribute("color", new THREE.Float32BufferAttribute(colors, 3));
  geometry.setIndex(indices);
  return geometry;
}
