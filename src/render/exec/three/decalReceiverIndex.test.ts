import { describe, expect, it } from "vitest";
import {
  BoxGeometry,
  BufferGeometry,
  Euler,
  Float32BufferAttribute,
  Mesh,
  PlaneGeometry,
  Vector3,
} from "three";
import { DecalGeometry } from "three/examples/jsm/geometries/DecalGeometry.js";
import { createDecalReceiverIndex } from "./decalReceiverIndex";

function arrays(geometry: BufferGeometry) {
  return Object.fromEntries(
    Object.entries(geometry.attributes).map(([key, attr]) => [
      key,
      Array.from(attr.array),
    ]),
  );
}
describe("decal receiver acceleration", () => {
  it("preserves exact clipping output for indexed and non-indexed transformed receivers", () => {
    for (const indexed of [true, false]) {
      const box = new BoxGeometry(10, 10, 10, 12, 12, 12);
      const mesh = new Mesh(indexed ? box : box.toNonIndexed());
      mesh.position.set(1, 2, 3);
      mesh.rotation.set(0.2, 0.4, 0.1);
      mesh.scale.set(1, 2, 0.8);
      mesh.updateMatrixWorld(true);
      const index = createDecalReceiverIndex();
      for (const position of [new Vector3(2, 2, 7), new Vector3(0, 0, 0)]) {
        const orientation = new Euler(0.1, 0.2, 0.3),
          size = new Vector3(4, 4, 12);
        const original = new DecalGeometry(mesh, position, orientation, size);
        const result = index.project(mesh, position, orientation, size);
        expect(result && arrays(result)).toEqual(arrays(original));
        result!.dispose();
        expect(
          arrays(index.project(mesh, position, orientation, size)!),
        ).toEqual(arrays(original));
      }
    }
  });
  it("invalidates modified vertex buffers and rejects invalid receivers", () => {
    const mesh = new Mesh(new PlaneGeometry(20, 20, 10, 10));
    const index = createDecalReceiverIndex();
    const position = new Vector3(),
      orientation = new Euler(),
      size = new Vector3(2, 2, 2);
    expect(index.project(mesh, position, orientation, size)).not.toBeNull();
    mesh.geometry.setAttribute(
      "position",
      new Float32BufferAttribute([NaN, 0, 0], 3),
    );
    expect(index.project(mesh, position, orientation, size)).toBeNull();
  });
  it("benchmarks repeated small decals on an 80,000 triangle receiver", () => {
    const mesh = new Mesh(new PlaneGeometry(200, 200, 200, 200));
    const orientation = new Euler(),
      size = new Vector3(2, 2, 2);
    const positions = Array.from(
      { length: 20 },
      (_, i) => new Vector3(i * 3 - 30, 1, 0),
    );
    const index = createDecalReceiverIndex();
    const start = performance.now();
    for (const position of positions)
      new DecalGeometry(mesh, position, orientation, size).dispose();
    const baseline = performance.now() - start;
    const coldStart = performance.now();
    for (const position of positions)
      index.project(mesh, position, orientation, size)?.dispose();
    const cold = performance.now() - coldStart;
    const warmStart = performance.now();
    for (const position of positions)
      index.project(mesh, position, orientation, size)?.dispose();
    const warm = performance.now() - warmStart;
    console.info("decal benchmark ms", { baseline, cold, warm });
    // Structural guard avoids a hardware-dependent timing assertion.
    expect(
      index.select(mesh, positions[0], orientation, size)!.geometry.index!
        .count,
    ).toBeLessThan(mesh.geometry.index!.count / 10);
  }, 30000);
});
