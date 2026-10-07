import * as THREE from "three";
import { existsSync } from "node:fs";
import { readdir } from "node:fs/promises";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import {
  collectNodeTransfers,
  extractObjectTree,
  nodesToObjects,
} from "./gltfParseDto";

/**
 * 真实 GLB 取证（2026-10-08 贴图乱码）：EP1 facade 网格（带 TEXCOORD_1
 * 材质索引/TEXCOORD_2·3 facade 双 UV 域）上对比「GLTFLoader 原始几何」与
 * 「worker 提取→transferable→主线程重建」的逐属性一致性。夹例缺失时跳过
 * （dump：cargo run -p sc-exporter --release --example dump_facade_glbs）。
 */
const FIXTURE_DIR = path.resolve("tmp/gltf_fixture");
const hasFixtures = existsSync(FIXTURE_DIR);

describe.skipIf(!hasFixtures)("真实 EP1 facade GLB roundtrip", () => {
  it("worker 提取/重建与 GLTFLoader 原始几何逐属性一致", async () => {
    const files = (await readdir(FIXTURE_DIR)).filter((f) =>
      f.endsWith(".glb"),
    );
    expect(files.length).toBeGreaterThan(0);

    const loader = new GLTFLoader();
    for (const file of files) {
      const glb = await import("node:fs/promises").then((fs) =>
        fs.readFile(path.join(FIXTURE_DIR, file)),
      );
      // node Buffer → ArrayBuffer 视图拷贝（GLTFLoader.parse 需要）
      const buffer = glb.buffer.slice(
        glb.byteOffset,
        glb.byteOffset + glb.byteLength,
      );
      const gltf = await loader.parseAsync(buffer, "");
      for (const child of gltf.scene.children) child.rotation.set(0, 0, 0);

      const nodes = extractObjectTree(gltf.scene, THREE);
      // 模拟 transfer（真实 worker 中跨 MessagePort，结构等价）
      void collectNodeTransfers(nodes);
      const rebuilt = nodesToObjects(nodes, THREE);
      expect(rebuilt).toHaveLength(1);

      const walk = (
        a: THREE.Object3D,
        b: THREE.Object3D,
        label: string,
      ): void => {
        expect(`${label}/${b.name}`).toBe(`${label}/${a.name}`);
        const ma = a as THREE.Mesh;
        const mb = b as THREE.Mesh;
        expect(mb.isMesh).toBe(ma.isMesh);
        if (ma.isMesh && mb.isMesh) {
          const names = new Set([
            ...Object.keys(ma.geometry.attributes),
            ...Object.keys(mb.geometry.attributes),
          ]);
          for (const name of names) {
            const attrA = ma.geometry.getAttribute(name);
            const attrB = mb.geometry.getAttribute(name);
            expect(attrB?.itemSize).toBe(attrA.itemSize);
            expect(attrB?.count).toBe(attrA.count);
            if (!attrA || !attrB) continue;
            expect(new Set(attrB.array)).toBeDefined();
            for (let i = 0; i < attrA.array.length; i += 1) {
              if (attrA.array[i] !== attrB.array[i]) {
                throw new Error(
                  `${label}/${name}[${i}]: ${attrA.array[i]} != ${attrB.array[i]}`,
                );
              }
            }
          }
          if (ma.geometry.index || mb.geometry.index) {
            const ia = ma.geometry.index?.array ?? [];
            const ib = mb.geometry.index?.array ?? [];
            expect(ib.length).toBe(ia.length);
            for (let i = 0; i < ia.length; i += 1) {
              if (ia[i] !== ib[i]) {
                throw new Error(`${label}/index[${i}]: ${ia[i]} != ${ib[i]}`);
              }
            }
          }
        }
        expect(a.children.length).toBe(b.children.length);
        for (let i = 0; i < a.children.length; i += 1) {
          walk(a.children[i], b.children[i], `${label}/child${i}`);
        }
      };
      walk(gltf.scene, rebuilt[0], file);
    }
  });

  it("uv1/uv2/uv3 数据存在且语义合理（EP1 facade 特征）", async () => {
    const files = (await readdir(FIXTURE_DIR)).filter((f) =>
      f.endsWith(".glb"),
    );
    const loader = new GLTFLoader();
    const glb = await import("node:fs/promises").then((fs) =>
      fs.readFile(path.join(FIXTURE_DIR, files[0])),
    );
    const buffer = glb.buffer.slice(
      glb.byteOffset,
      glb.byteOffset + glb.byteLength,
    );
    const gltf = await loader.parseAsync(buffer, "");
    let uv1: THREE.BufferAttribute | THREE.InterleavedBufferAttribute | undefined;
    let uv2: THREE.BufferAttribute | THREE.InterleavedBufferAttribute | undefined;
    gltf.scene.traverse((child) => {
      const mesh = child as THREE.Mesh;
      if (mesh.isMesh && !uv1) {
        uv1 = mesh.geometry.getAttribute("uv1");
        uv2 = mesh.geometry.getAttribute("uv2");
      }
    });
    expect(uv1).toBeDefined();
    expect(uv1!.itemSize).toBe(4);
    // uv1.x = materialIndex/255 ∈ [0,1]
    for (let i = 0; i < uv1!.count; i += 1) {
      expect(uv1!.getX(i)).toBeGreaterThanOrEqual(0);
      expect(uv1!.getX(i)).toBeLessThanOrEqual(1);
    }
    expect(uv2).toBeDefined();
    expect(uv2!.itemSize).toBe(2);
  });
});
