import { describe, expect, it } from "vitest";
import { parseLotModelContainer } from "./three-gltf";

/** 按 Rust 侧 v2 容器布局构建字节（小端）。 */
function buildPayload(
  glbSizes: number[],
  materials: { baseColorSize: number; normalSize: number }[],
  meshMaterialIndices: number[],
  meshHasUv: boolean[],
): ArrayBuffer {
  const materialBytes = materials.reduce(
    (total, material) => total + 8 + material.baseColorSize + material.normalSize,
    0,
  );
  const size =
    12 +
    glbSizes.reduce((total, length) => total + 4 + length, 0) +
    4 +
    materialBytes +
    meshMaterialIndices.length * 5;
  const buffer = new ArrayBuffer(size);
  const view = new DataView(buffer);
  let offset = 0;
  const u32 = (value: number) => {
    view.setUint32(offset, value, true);
    offset += 4;
  };
  u32(0x4d544f4c); // "LOTM"
  u32(2);
  u32(glbSizes.length);
  glbSizes.forEach((length, index) => {
    u32(length);
    new Uint8Array(buffer, offset, length).fill(index + 1);
    offset += length;
  });
  u32(materials.length);
  for (const material of materials) {
    u32(material.baseColorSize);
    offset += material.baseColorSize;
    u32(material.normalSize);
    offset += material.normalSize;
  }
  meshMaterialIndices.forEach((materialIndex, index) => {
    u32(materialIndex);
    view.setUint8(offset, meshHasUv[index] ? 1 : 0);
    offset += 1;
  });
  return buffer;
}

describe("parseLotModelContainer", () => {
  it("解析双 GLB + 双材质 + 逐 mesh 属性", () => {
    const payload = parseLotModelContainer(
      buildPayload(
        [8, 16],
        [
          { baseColorSize: 32, normalSize: 64 },
          { baseColorSize: 0, normalSize: 12 },
        ],
        [1, 0],
        [true, false],
      ),
    );
    expect(payload.glbs).toHaveLength(2);
    expect(payload.glbs[0].byteLength).toBe(8);
    expect(new Uint8Array(payload.glbs[1])[0]).toBe(2);
    expect(payload.materials).toHaveLength(2);
    expect(payload.materials[0].baseColorPng?.byteLength).toBe(32);
    expect(payload.materials[0].normalPng?.byteLength).toBe(64);
    expect(payload.materials[1].baseColorPng).toBeNull();
    expect(payload.materials[1].normalPng?.byteLength).toBe(12);
    expect(payload.meshMaterialIndices).toEqual([1, 0]);
    expect(payload.meshHasUv).toEqual([true, false]);
  });

  it("零材质单 mesh 容器", () => {
    const payload = parseLotModelContainer(buildPayload([4], [], [0], [false]));
    expect(payload.glbs).toHaveLength(1);
    expect(payload.materials).toHaveLength(0);
    expect(payload.meshMaterialIndices).toEqual([0]);
    expect(payload.meshHasUv).toEqual([false]);
  });

  it("魔数不符与截断容器抛错", () => {
    const good = buildPayload([], [], [], []);
    new DataView(good).setUint32(0, 0x12345678, true);
    expect(() => parseLotModelContainer(good)).toThrow("magic mismatch");
    expect(() => parseLotModelContainer(new ArrayBuffer(4))).toThrow("truncated");
  });
});
