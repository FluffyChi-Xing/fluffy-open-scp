import { describe, expect, it } from "vitest";
import {
  inspectGlb,
  normalizeLotModelScene,
  parseLotModelContainer,
  parseLotModelObjects,
  rotationXForUpAxis,
} from "./three-gltf";

/** 按 Rust 侧 LOTM 容器布局构建字节（小端）——先压入数组再一次性分配。 */
interface MaterialSpec {
  baseColorSize: number;
  normalSize: number;
  roughnessSize: number;
  aoSize: number;
  tintSize: number;
  paletteSize: number;
  shaderSize: number;
  interiorSize: number;
  reliefSize: number;
  paramsFloats: number;
}

function buildPayload(
  glbSizes: number[],
  materials: MaterialSpec[],
  meshMaterialIndices: number[],
  meshUvKinds: number[],
  diagnostics = "",
  version: 8 | 9 = 9,
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
  // v9 = 8 PNG/材质（relief 停发）；v8 = 9 PNG/材质（历史格式，前端兼容读取）。
  const pngsPerMaterial = version >= 9 ? 8 : 9;

  u32(0x4d544f4c); // "LOTM"
  u32(version);
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
    png(material.shaderSize, 7);
    png(material.interiorSize, 8);
    if (pngsPerMaterial >= 9) png(material.reliefSize, 9);
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
  const diagBytes = new TextEncoder().encode(diagnostics);
  u32(diagBytes.length);
  for (const byte of diagBytes) bytes.push(byte);
  return new Uint8Array(bytes).buffer;
}

describe("parseLotModelContainer", () => {
  it("解析 v9 双 GLB + 双材质 + 逐 mesh uvKind（relief 已停发）", () => {
    const payload = parseLotModelContainer(
      buildPayload(
        [8, 16],
        [
          { baseColorSize: 32, normalSize: 64, roughnessSize: 16, aoSize: 16, tintSize: 48, paletteSize: 128, shaderSize: 96, interiorSize: 112, reliefSize: 0, paramsFloats: 8 },
          { baseColorSize: 0, normalSize: 12, roughnessSize: 0, aoSize: 8, tintSize: 0, paletteSize: 0, shaderSize: 0, interiorSize: 0, reliefSize: 0, paramsFloats: 0 },
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
    expect(payload.materials[0].shaderPng?.byteLength).toBe(96);
    expect(payload.materials[0].interiorPng?.byteLength).toBe(112);
    expect(payload.materials[1].shaderPng).toBeNull();
    expect(payload.materials[1].interiorPng).toBeNull();
    // v9 停发 relief：字段保留（类型兼容）但恒为 null
    expect(payload.materials[0].reliefPng).toBeNull();
    expect(payload.materials[0].paramsF32?.length).toBe(8);
    expect(payload.materials[0].paramCols).toBe(2);
    expect(payload.materials[0].paramsF32?.[3]).toBe(4);
    expect(payload.materials[1].baseColorPng).toBeNull();
    expect(payload.materials[1].tintPng).toBeNull();
    expect(payload.materials[1].paramsF32).toBeNull();
    expect(payload.meshMaterialIndices).toEqual([1, 0]);
    expect(payload.meshUvKinds).toEqual([2, 1]);
    expect(payload.diagnostics).toBe("");
  });

  it("兼容解析历史 v8 容器（9 PNG/材质，relief 读取即弃）", () => {
    const payload = parseLotModelContainer(
      buildPayload(
        [4],
        [
          { baseColorSize: 8, normalSize: 8, roughnessSize: 8, aoSize: 8, tintSize: 8, paletteSize: 8, shaderSize: 8, interiorSize: 8, reliefSize: 64, paramsFloats: 0 },
        ],
        [0],
        [0],
      ),
    );
    expect(payload.materials).toHaveLength(1);
    expect(payload.materials[0].interiorPng?.byteLength).toBe(8);
    expect(payload.materials[0].reliefPng).toBeNull();
    expect(payload.meshUvKinds).toEqual([0]);
  });

  it("零材质单 mesh 容器", () => {
    const payload = parseLotModelContainer(buildPayload([4], [], [0], [0]));
    expect(payload.glbs).toHaveLength(1);
    expect(payload.materials).toHaveLength(0);
    expect(payload.meshMaterialIndices).toEqual([0]);
    expect(payload.meshUvKinds).toEqual([0]);
  });

  it("解析末尾诊断文本", () => {
    const payload = parseLotModelContainer(
      buildPayload([4], [], [0], [0], "mesh #5 → material #6\nslot1 0x00000001 [a.package raster]"),
    );
    expect(payload.diagnostics).toBe(
      "mesh #5 → material #6\nslot1 0x00000001 [a.package raster]",
    );
  });

  it("魔数不符与截断容器抛错", () => {
    const good = buildPayload([], [], [], []);
    new DataView(good).setUint32(0, 0x12345678, true);
    expect(() => parseLotModelContainer(good)).toThrow("magic mismatch");
    expect(() => parseLotModelContainer(new ArrayBuffer(4))).toThrow("truncated");
  });
});

describe("imported model axis", () => {
  it("keeps Z-up Blender assets for the shared world transform", () => {
    expect(rotationXForUpAxis("z")).toBe(0);
    expect(rotationXForUpAxis("y")).toBeCloseTo(Math.PI / 2);
    expect(rotationXForUpAxis(undefined)).toBe(0);
  });

  /**
   * 原生口径 GLB：Z-up 建筑数据（底面 z=0）+ 根节点 ∓90°X（gltf.rs /
   * LOTM 容器 / PE 导出的口径）。渲染契约 = parseLotModelObjects 的产物
   * 放进共享 world（-90°X）；量测契约 = measureFootprint 的同源规范化。
   * 2026-10-26 draft 悬空回归：量测侧漏剥根旋转，modelOrigin 在错误的
   * 帧里取 -min.z，把建筑垫高了足迹深度的一半。
   */
  async function nativeStyleGlb(): Promise<ArrayBuffer> {
    const THREE = await import("three");
    const { GLTFExporter } = await import(
      "three/examples/jsm/exporters/GLTFExporter.js"
    );
    const data = new THREE.Group();
    const geometry = new THREE.BoxGeometry(48, 34, 37);
    geometry.translate(0, 0, 18.5);
    data.add(new THREE.Mesh(geometry));
    const root = new THREE.Group();
    root.rotation.x = -Math.PI / 2;
    root.add(data);
    return await new Promise<ArrayBuffer>((resolve, reject) =>
      new GLTFExporter().parse(
        root,
        (out) => resolve(out as ArrayBuffer),
        reject,
        { binary: true },
      ),
    );
  }

  /** 渲染最终视图坐标（parse 契约 + 可选 origin 平移 + world -90°X）。 */
  async function renderedViewBox(
    glb: ArrayBuffer,
    origin?: [number, number, number],
  ): Promise<{ min: number[]; max: number[] }> {
    const THREE = await import("three");
    const objects = await parseLotModelObjects([glb], undefined, false);
    if (origin) objects[0].position.add(new THREE.Vector3(...origin));
    const world = new THREE.Group();
    world.rotation.x = -Math.PI / 2;
    world.add(objects[0]);
    world.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(world);
    return { min: box.min.toArray(), max: box.max.toArray() };
  }

  /** 量测契约（measureFootprint 的同源规范化 + bbox → origin）。 */
  async function measuredOrigin(
    glb: ArrayBuffer,
  ): Promise<[number, number, number]> {
    const THREE = await import("three");
    const { GLTFLoader } = await import(
      "three/examples/jsm/loaders/GLTFLoader.js"
    );
    const { scene } = await new GLTFLoader().parseAsync(glb, "");
    normalizeLotModelScene(scene, undefined, { declared: false });
    scene.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(scene);
    return [
      -(box.min.x + box.max.x) / 2,
      -(box.min.y + box.max.y) / 2,
      -box.min.z,
    ];
  }

  it(
    "grounds a native lotm draft: measure and render share one frame",
    async () => {
      const glb = await nativeStyleGlb();
      const origin = await measuredOrigin(glb);
      // 原点必须在渲染帧里取值：居中 + 底面坐 z=0（此前错误帧给 [0,-18.5,17]）。
      expect(origin[0]).toBeCloseTo(0, 1);
      expect(origin[1]).toBeCloseTo(0, 1);
      expect(origin[2]).toBeCloseTo(0, 1);
      const view = await renderedViewBox(glb, origin);
      // 装配后底面落在地面（y=0），高度 37 保持竖立。
      expect(view.min[1]).toBeCloseTo(0, 1);
      expect(view.max[1]).toBeCloseTo(37, 1);
    },
    20000,
  );

  it("keeps the render contract grounded without an origin shift", async () => {
    const view = await renderedViewBox(await nativeStyleGlb());
    expect(view.min[1]).toBeCloseTo(0, 1);
    expect(view.max[1]).toBeCloseTo(37, 1);
  }, 20000);

  /** 最小 GLB 容器（仅 asset.extras + POSITION 包围盒，供轴向判定）。 */
  function axisFixture(axis?: unknown): ArrayBuffer {
    const json = JSON.stringify({
      asset: { version: "2.0", extras: { openscp: { upAxis: axis } } },
      scenes: [{ nodes: [0] }], nodes: [{ mesh: 0 }],
      meshes: [{ primitives: [{ attributes: { POSITION: 0, TEXCOORD_0: 1 } }] }],
      accessors: [{ min: [0, 0, 0], max: [20, 2, 30] }],
    });
    const text = new TextEncoder().encode(json);
    const length = (text.length + 3) & ~3;
    const bytes = new Uint8Array(20 + length);
    bytes.fill(32, 20);
    const view = new DataView(bytes.buffer);
    [0x46546c67, 2, bytes.length, length, 0x4e4f534a].forEach((v, i) =>
      view.setUint32(i * 4, v, true),
    );
    bytes.set(text, 20);
    return bytes.buffer;
  }
  // 低矮而纵深大的建筑在合法 Y-up GLB 里也可能 Z 跨度最大；显式声明
  // （asset.extras.openscp.upAxis，NY Facade 写入 / blend 转换注入）必须
  // 优先于尺寸猜测，无声明或非法值回退原判定。
  it("honors a declared Y-up for a low deep building instead of guessing", () => {
    const declared = inspectGlb(axisFixture("y"));
    expect(declared.upAxis).toBe("y");
    expect(declared.upAxisBasis).toBe(
      "asset.extras.openscp.upAxis=y",
    );
    expect(declared.upAxisDeclared).toBe(true);
    expect(inspectGlb(axisFixture("z")).upAxis).toBe("z");
    expect(inspectGlb(axisFixture("z")).upAxisDeclared).toBe(true);
  });
  it("keeps legacy inference for absent or invalid axis metadata", () => {
    expect(inspectGlb(axisFixture()).upAxis).toBe("z");
    expect(inspectGlb(axisFixture()).upAxisDeclared).toBe(false);
    expect(inspectGlb(axisFixture("invalid")).upAxis).toBe("z");
  });
});
