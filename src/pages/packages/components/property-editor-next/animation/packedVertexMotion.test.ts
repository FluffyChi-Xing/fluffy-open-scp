import { BufferGeometry, Float32BufferAttribute } from "three";
import { expect, it } from "vitest";
import { createPackedVertexSampler } from "./packedVertexMotion";

it("rotates parts sharing an animation slot around their own pivots and preserves other slots", () => {
  const geometry = new BufferGeometry();
  const position = new Float32BufferAttribute([1, 0, 0, 11, 0, 0, 21, 0, 0, 5, 6, 7], 3);
  geometry.setAttribute("position", position);
  const packed = new ArrayBuffer(48);
  const bytes = new DataView(packed);
  for (const [i, pivot] of [0, 10, 20].entries()) {
    new Uint8Array(packed, i * 12, 4).set([127, 127, 254, i === 2 ? 1 : 0]);
    bytes.setInt16(i * 12 + 4, pivot * 64, true);
    bytes.setInt16(i * 12 + 10, Math.round(Math.PI * 512), true);
  }
  // A neutral axis may still translate: this is how the industrial model
  // encodes several moving parts on slot zero.
  new Uint8Array(packed, 36, 4).set([127, 127, 127, 0]);
  bytes.setInt16(44, -128, true);
  const sample = createPackedVertexSampler(geometry, packed, 0)!;
  sample(1);
  expect(position.getX(0)).toBeCloseTo(0, 2);
  expect(position.getY(0)).toBeCloseTo(1, 2);
  expect(position.getX(1)).toBeCloseTo(10, 2);
  expect(position.getY(1)).toBeCloseTo(1, 2);
  expect(position.getX(2)).toBe(21);
  expect(position.getZ(3)).toBe(5);
  sample(0);
  expect(Array.from(position.array)).toEqual([1, 0, 0, 11, 0, 0, 21, 0, 0, 5, 6, 7]);
});
