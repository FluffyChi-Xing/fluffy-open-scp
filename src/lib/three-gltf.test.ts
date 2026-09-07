import { describe, expect, it } from "vitest";
import { parseLotModelContainer } from "./three-gltf";

/** 按 Rust 侧 v4 容器布局构建字节（小端）——先压入数组再一次性分配。 */
interface MaterialSpec {
  baseColorSize: number;
  normalSize: number;
  roughnessSize: number;
  aoSize: number;
  tintSize: number;
  paletteSize: number;
  paramsFloats: number;
}

function buildPayload(
  glbSizes: number[],
  materials: MaterialSpec[],
  meshMaterialIndices: number[],
  meshUvKinds: number[],
): ArrayBuffer {
  const bytes: number[] = [];
  const u32 = (value: number) => {
    bytes.push(value & 0xff, (value >>> 8) & 0xff, (value >>> 16) & 0xff, (value >>> 24) & 0xff);
  };
  const pushF32 = (value: number) => {
    const view = new DataView(new ArrayBuffer(4));
    view.setFloat32(0, value, true);
    for (let i = 0; i < 4; i += 1) bytes.push(view.getUint8(i));
  };
  const png = (size: number, fill: number) => {
    u32(size);
    for (let i = 0; i < size; i += 1) bytes.push(fill);
  };
  const f4 = (values: number[]) => {
    u32(values.length * 4);
    for (const value of values) pushF32(value);
  };

  u32(0x4d544f4c); // "LOTM"
  u32(4);
  u32(glbSizes.length);
  glbSizes.forEach((length, index) => {
    u32(length);
    for (let i = 0; i < length; i += 1) bytes.push(index + 1);
  });
  u32(materials.length);
  for (const material of materials) {
    png(material.baseColorSize, 1);
    png(material.normalSize, 2);
    png(material.roughnessSize, 3);
    png(material.aoSize, 4);
    png(material.tintSize, 5);
    png(material.paletteSize, 6);
    const params = Array.from(
      { length: material.paramsFloats },
      (_, i) => i + 1,
    );
    f4(params);
    u32(material.paramsFloats / 4);
  }
  meshMaterialIndices.forEach((materialIndex, index) => {
    u32(materialIndex);
    bytes.push(meshUvKinds[index]);
  });
  return new Uint8Array(bytes).buffer;
}

describe("parseLotModelContainer", () => {
  it("解析双 GLB + 双材质 + 逐 mesh uvKind", () => {
    const payload = parseLotModelContainer(
      buildPayload(
        [8, 16],
        [
          { baseColorSize: 32, normalSize: 64, roughnessSize: 16, aoSize: 16, tintSize: 48, paletteSize: 128, paramsFloats: 8 },
          { baseColorSize: 0, normalSize: 12, roughnessSize: 0, aoSize: 8, tintSize: 0, paletteSize: 0, paramsFloats: 0 },
        ],
        [1, 0],
        [2, 1],
      ),
    );
    expect(payload.glbs).toHaveLength(2);
    expect(payload.glbs[0].byteLength).toBe(8);
    expect(new Uint8Array(payload.glbs[1])[0]).toBe(2);
    expect(payload.materials).toHaveLength(2);
    expect(payload.materials[0].baseColorPng?.byteLength).toBe(32);
    expect(payload.materials[0].normalPng?.byteLength).toBe(64);
    expect(payload.materials[0].roughnessPng?.byteLength).toBe(16);
    expect(payload.materials[0].aoPng?.byteLength).toBe(16);
    expect(payload.materials[0].tintPng?.byteLength).toBe(48);
    expect(payload.materials[0].palettePng?.byteLength).toBe(128);
    expect(payload.materials[0].paramsF32?.length).toBe(8);
    expect(payload.materials[0].paramCols).toBe(2);
    expect(payload.materials[0].paramsF32?.[3]).toBe(4);
    expect(payload.materials[1].baseColorPng).toBeNull();
    expect(payload.materials[1].tintPng).toBeNull();
    expect(payload.materials[1].paramsF32).toBeNull();
    expect(payload.meshMaterialIndices).toEqual([1, 0]);
    expect(payload.meshUvKinds).toEqual([2, 1]);
  });

  it("零材质单 mesh 容器", () => {
    const payload = parseLotModelContainer(buildPayload([4], [], [0], [0]));
    expect(payload.glbs).toHaveLength(1);
    expect(payload.materials).toHaveLength(0);
    expect(payload.meshMaterialIndices).toEqual([0]);
    expect(payload.meshUvKinds).toEqual([0]);
  });

  it("魔数不符与截断容器抛错", () => {
    const good = buildPayload([], [], [], []);
    new DataView(good).setUint32(0, 0x12345678, true);
    expect(() => parseLotModelContainer(good)).toThrow("magic mismatch");
    expect(() => parseLotModelContainer(new ArrayBuffer(4))).toThrow("truncated");
  });
});
