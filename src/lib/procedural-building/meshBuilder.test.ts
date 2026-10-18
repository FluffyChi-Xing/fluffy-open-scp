import { describe, expect, it } from "vitest";
import {
  appendBox,
  buildModelPayload,
  buildSlotGeometries,
  defaultSlotColor,
  hexToLinear,
} from "./meshBuilder";
import { type BuildingPart } from "./schema";
import { parseLotModelContainer } from "@/lib/three-gltf";

function boxPart(
  id: string,
  slot: BuildingPart["slot"],
  center: [number, number, number],
  size: [number, number, number],
): BuildingPart {
  return { id, kind: "wall", slot, center, size };
}

function newArrays(): {
  position: number[];
  normal: number[];
  uv: number[];
  color: number[];
  index: number[];
} {
  return { position: [], normal: [], uv: [], color: [], index: [] };
}

describe("procedural building mesh builder", () => {
  it("maps sRGB hex to linear RGB", () => {
    expect(hexToLinear("#ffffff")).toEqual([1, 1, 1]);
    expect(hexToLinear("#000000")).toEqual([0, 0, 0]);
    const [r, g, b] = hexToLinear("#808080");
    expect(r).toBeCloseTo(0.2158, 3);
    expect(g).toBeCloseTo(0.2158, 3);
    expect(b).toBeCloseTo(0.2158, 3);
  });

  it("stretches each face over the full box extent with one full-texture UV", () => {
    const host = newArrays();
    appendBox(host, [0, 0, 0], [6, 2, 3], [1, 1, 1]);
    // 6 面 × 4 顶点（无细分）
    expect(host.position.length / 3).toBe(6 * 4);
    expect(host.index.length / 3).toBe(6 * 2);
    // 顶点范围铺满整个盒体（±sx/2, ±sy/2, ±sz/2）——回归：单位 u/v
    // 曾把所有面退化成 1m 角片
    let minX = Infinity, maxX = -Infinity;
    let minY = Infinity, maxY = -Infinity;
    let minZ = Infinity, maxZ = -Infinity;
    for (let i = 0; i < host.position.length; i += 3) {
      minX = Math.min(minX, host.position[i]);
      maxX = Math.max(maxX, host.position[i]);
      minY = Math.min(minY, host.position[i + 1]);
      maxY = Math.max(maxY, host.position[i + 1]);
      minZ = Math.min(minZ, host.position[i + 2]);
      maxZ = Math.max(maxZ, host.position[i + 2]);
    }
    expect(minX).toBeCloseTo(-3, 4);
    expect(maxX).toBeCloseTo(3, 4);
    expect(minY).toBeCloseTo(-1, 4);
    expect(maxY).toBeCloseTo(1, 4);
    expect(minZ).toBeCloseTo(-1.5, 4);
    expect(maxZ).toBeCloseTo(1.5, 4);
    // 每面的 UV 都是完整四角 (0,0)(1,0)(1,1)(0,1)——游戏 atlas 口径
    for (let face = 0; face < 6; face += 1) {
      const base = face * 4 * 2;
      expect(host.uv.slice(base, base + 8)).toEqual([0, 0, 1, 0, 1, 1, 0, 1]);
    }
    // 法线全部为单位轴向量
    for (let i = 0; i < host.normal.length; i += 3) {
      const length = Math.hypot(host.normal[i], host.normal[i + 1], host.normal[i + 2]);
      expect(length).toBeCloseTo(1, 5);
    }
  });

  it("groups parts by material slot", () => {
    const parts = [
      boxPart("a", "wall", [0, 0, 0], [2, 2, 2]),
      boxPart("b", "wall", [3, 0, 0], [2, 2, 2]),
      boxPart("c", "trim", [6, 0, 0], [1, 1, 1]),
    ];
    const geometries = buildSlotGeometries(parts);
    expect([...geometries.keys()].sort()).toEqual(["trim", "wall"]);
    const wall = geometries.get("wall")!;
    // 2 个盒体 × 6 面 × 4 顶点
    expect(wall.getAttribute("position")!.count).toBe(2 * 6 * 4);
  });

  it("uses white vertex color for textured slots and default color otherwise", () => {
    expect(defaultSlotColor("wall", true)).toEqual([1, 1, 1]);
    const untextured = defaultSlotColor("wall", false);
    expect(untextured[0]).toBeLessThan(1);
    expect(untextured).toHaveLength(3);
  });

  it("builds a LotModelPayload consumable by the LOTM payload shape", async () => {
    const parts = [
      boxPart("a", "wall", [0, 0, 0], [2, 2, 2]),
      boxPart("b", "roof", [3, 0, 0], [1, 1, 1]),
    ];
    const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
    const payload = await buildModelPayload({
      parts,
      textures: new Map([["wall", png]]),
      textureLabels: new Map([["wall", "test-raster"]]),
    });
    expect(payload.glbs).toHaveLength(2);
    expect(payload.meshMaterialIndices).toEqual([0, 1]);
    expect(payload.meshUvKinds).toEqual([1, 1]);
    expect(payload.materials[0].baseColorPng).toBe(png);
    expect(payload.materials[1].baseColorPng).toBeNull();
    expect(payload.diagnostics).toContain("wall: test-raster");
    // GLB 魔数（glTF 二进制容器）
    expect(new DataView(payload.glbs[0]).getUint32(0, true)).toBe(0x46546c67);
  }, 20000);

  it("round-trips the generated payload through the LOTM container parser", async () => {
    const parts = [boxPart("a", "roof", [0, 0, 0], [2, 2, 2])];
    const payload = await buildModelPayload({
      parts,
      textures: new Map(),
    });
    const repacked = parseLotModelContainer(repackForTest(payload));
    expect(repacked.glbs.length).toBe(payload.glbs.length);
    expect(repacked.meshUvKinds).toEqual(payload.meshUvKinds);
  }, 20000);
});

/** payload → LOTM 容器字节（复刻后端 v9 布局，验证前端解析兼容）。 */
function repackForTest(payload: {
  glbs: ArrayBuffer[];
  materials: { baseColorPng: Uint8Array<ArrayBuffer> | null }[];
  meshMaterialIndices: number[];
  meshUvKinds: number[];
  diagnostics: string;
}): ArrayBuffer {
  const chunks: Uint8Array[] = [];
  const view = (bytes: Uint8Array) => chunks.push(bytes);
  const u32 = (value: number) => {
    const bytes = new Uint8Array(4);
    new DataView(bytes.buffer).setUint32(0, value, true);
    view(bytes);
  };
  u32(0x4d54_4f4c);
  u32(9);
  u32(payload.glbs.length);
  for (const glb of payload.glbs) {
    u32(glb.byteLength);
    view(new Uint8Array(glb));
  }
  u32(payload.materials.length);
  for (const material of payload.materials) {
    // v9 = 每材质 8 张 PNG（relief 停发）
    for (let png = 0; png < 8; png += 1) {
      const bytes = material.baseColorPng;
      u32(bytes ? bytes.byteLength : 0);
      if (bytes) view(bytes);
    }
    u32(0); // paramsF32
    u32(0); // paramCols
  }
  for (let i = 0; i < payload.glbs.length; i += 1) {
    u32(payload.meshMaterialIndices[i] ?? 0);
    const kind = new Uint8Array(1);
    kind[0] = payload.meshUvKinds[i] ?? 0;
    view(kind);
  }
  const diag = new TextEncoder().encode(payload.diagnostics);
  u32(diag.byteLength);
  view(diag);
  const total = chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return out.buffer;
}
