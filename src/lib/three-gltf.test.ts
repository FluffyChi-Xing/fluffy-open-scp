import { describe, expect, it } from "vitest";
import { parseLotModelContainer } from "./three-gltf";

/** 按 Rust 侧容器布局构建字节（小端）。 */
function buildPayload(
  glbSizes: number[],
  baseColorSize: number,
  normalSize: number,
  hasUv: boolean,
): ArrayBuffer {
  const size =
    12 +
    glbSizes.reduce((total, length) => total + 4 + length, 0) +
    4 +
    baseColorSize +
    4 +
    normalSize +
    1;
  const buffer = new ArrayBuffer(size);
  const view = new DataView(buffer);
  let offset = 0;
  const u32 = (value: number) => {
    view.setUint32(offset, value, true);
    offset += 4;
  };
  u32(0x4d544f4c); // "LOTM"
  u32(1);
  u32(glbSizes.length);
  glbSizes.forEach((length, index) => {
    u32(length);
    new Uint8Array(buffer, offset, length).fill(index + 1);
    offset += length;
  });
  u32(baseColorSize);
  offset += baseColorSize;
  u32(normalSize);
  offset += normalSize;
  view.setUint8(offset, hasUv ? 1 : 0);
  return buffer;
}

describe("parseLotModelContainer", () => {
  it("解析多 GLB + 双贴图容器", () => {
    const payload = parseLotModelContainer(buildPayload([8, 16], 32, 64, true));
    expect(payload.glbs).toHaveLength(2);
    expect(payload.glbs[0].byteLength).toBe(8);
    expect(new Uint8Array(payload.glbs[1])[0]).toBe(2);
    expect(payload.baseColorPng?.byteLength).toBe(32);
    expect(payload.normalPng?.byteLength).toBe(64);
    expect(payload.hasUv).toBe(true);
  });

  it("零长度贴图解析为 null", () => {
    const payload = parseLotModelContainer(buildPayload([4], 0, 0, false));
    expect(payload.glbs).toHaveLength(1);
    expect(payload.baseColorPng).toBeNull();
    expect(payload.normalPng).toBeNull();
    expect(payload.hasUv).toBe(false);
  });

  it("魔数不符与截断容器抛错", () => {
    const good = buildPayload([], 0, 0, false);
    new DataView(good).setUint32(0, 0x12345678, true);
    expect(() => parseLotModelContainer(good)).toThrow("magic mismatch");
    expect(() => parseLotModelContainer(new ArrayBuffer(4))).toThrow("truncated");
  });
});
