import type * as ThreeNamespace from "three";
import type { LotModelPayload } from "@/api/tauri";

/** 与 Rust 侧 `LOT_MODEL_PAYLOAD_MAGIC` 一致："LOTM"（小端字节序）。 */
const PAYLOAD_MAGIC = 0x4d54_4f4c;

/**
 * 解析 `read_lot_model_meshes` 返回的原始字节容器（零拷贝切片，
 * 见 `LotModelPayload` 注释里的布局：逐 mesh GLB + 逐材质贴图 +
 * 每 mesh 材质下标/uv 类型 + 诊断文本）。
 *
 * v8 = 9 PNG/材质（含已废弃的 relief）；v9 = 8 PNG/材质（relief 停发——
 * 前端视差已回滚、该图从不加载，逐材质省一张 PNG 的 IPC）。两者都接受。
 */
export function parseLotModelContainer(buffer: ArrayBuffer): LotModelPayload {
  const view = new DataView(buffer);
  if (buffer.byteLength < 16) {
    throw new Error("lot model payload truncated");
  }
  let offset = 0;
  const readU32 = () => {
    const value = view.getUint32(offset, true);
    offset += 4;
    return value;
  };
  if (readU32() !== PAYLOAD_MAGIC) {
    throw new Error("lot model payload magic mismatch");
  }
  const version = readU32();
  if (version !== 8 && version !== 9) {
    throw new Error(`unsupported lot model payload version ${version}`);
  }
  const pngsPerMaterial = version >= 9 ? 8 : 9;
  const meshCount = readU32();
  const glbs: ArrayBuffer[] = [];
  for (let index = 0; index < meshCount; index += 1) {
    const length = readU32();
    if (offset + length > buffer.byteLength) {
      throw new Error("lot model payload mesh out of bounds");
    }
    glbs.push(buffer.slice(offset, offset + length));
    offset += length;
  }
  const materialCount = readU32();
  const materials: LotModelPayload["materials"] = [];
  for (let index = 0; index < materialCount; index += 1) {
    const readPng = (): Uint8Array<ArrayBuffer> | null => {
      const length = readU32();
      if (length === 0) return null;
      if (offset + length > buffer.byteLength) {
        throw new Error("lot model payload texture out of bounds");
      }
      const bytes = buffer.slice(offset, offset + length);
      offset += length;
      return new Uint8Array(bytes);
    };
    const pngs: (Uint8Array<ArrayBuffer> | null)[] = [];
    for (let png = 0; png < pngsPerMaterial; png += 1) pngs.push(readPng());
    const [baseColorPng, normalPng, roughnessPng, aoPng, tintPng, palettePng, shaderPng, interiorPng] =
      pngs;
    // v8 的第 9 张（relief）读取后即弃：字段仅为类型兼容保留，无消费方。
    const paramsLength = readU32();
    let paramsF32: Float32Array | null = null;
    if (paramsLength > 0) {
      if (offset + paramsLength > buffer.byteLength) {
        throw new Error("lot model payload params out of bounds");
      }
      paramsF32 = new Float32Array(buffer.slice(offset, offset + paramsLength));
      offset += paramsLength;
    }
    const paramCols = readU32();
    materials.push({
      baseColorPng,
      normalPng,
      roughnessPng,
      aoPng,
      tintPng,
      palettePng,
      shaderPng,
      interiorPng,
      reliefPng: null,
      paramsF32,
      paramCols,
    });
  }
  const meshMaterialIndices: number[] = [];
  const meshUvKinds: number[] = [];
  for (let index = 0; index < meshCount; index += 1) {
    if (offset + 5 > buffer.byteLength) {
      throw new Error("lot model payload mesh attributes truncated");
    }
    meshMaterialIndices.push(readU32());
    meshUvKinds.push(view.getUint8(offset));
    offset += 1;
  }
  let diagnostics = "";
  if (offset + 4 <= buffer.byteLength) {
    const diagLength = readU32();
    if (offset + diagLength > buffer.byteLength) {
      throw new Error("lot model payload diagnostics out of bounds");
    }
    diagnostics = new TextDecoder().decode(
      buffer.slice(offset, offset + diagLength),
    );
  }
  return { glbs, materials, meshMaterialIndices, meshUvKinds, diagnostics };
}

/** PNG 字节 → blob URL（TextureLoader 可直接加载，免去 data:URL base64 再解码）。 */
export function pngBlobUrl(bytes: Uint8Array<ArrayBuffer>): string {
  return URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
}

/**
 * 解析地块模型的全部 GLB 为 Object3D。
 * gltf.rs 根节点自带 Z-up→Y-up 的 -90°X 旋转，而视口 world 组已做同款
 * 旋转（模型须与 Unit gizmo 共享 Z-up 世界），这里剥掉根旋转避免双重旋转。
 */
export async function parseLotModelObjects(
  glbs: ArrayBuffer[],
): Promise<ThreeNamespace.Object3D[]> {
  const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
  const loader = new GLTFLoader();
  const objects = await Promise.all(
    glbs.map(async (glb) => {
      const gltf = await loader.parseAsync(glb, "");
      for (const child of gltf.scene.children) {
        child.rotation.set(0, 0, 0);
      }
      return gltf.scene;
    }),
  );
  return objects;
}

/**
 * payload 级模型对象缓存：同一 payload（renderMode/grouping 变化引起的
 * 重复 rebuild 是热路径）只 parse 一次，后续 rebuild 取 `clone()`——
 * clone 共享 geometry/材质引用，成本是场景图节点数而非几何数据量。
 *
 * 缓存的 geometry 由 `markSharedGeometry` 打标，`disposeObject` 跳过
 * （rebuild 清场不会销毁它们）；payload 更换时 `releaseLotModelCache`
 * 显式解除标记并 dispose，避免 GPU 资源泄漏。
 */
const sharedGeometries = new WeakSet<ThreeNamespace.BufferGeometry>();
/** 当前持有的缓存 payload（WeakMap 不负责 GPU 资源，需显式释放旧代）。 */
let cachedPayload: LotModelPayload | null = null;
let cachedRoots: ThreeNamespace.Object3D[] = [];

/** clone 后入场的几何共享标记（disposeObject 跳过用）。 */
export function isSharedGeometry(
  geometry: ThreeNamespace.BufferGeometry | undefined,
): boolean {
  return geometry !== undefined && sharedGeometries.has(geometry);
}

/** 标记一块跨 rebuild 共享的几何（如 decal 投影缓存），清场时不 dispose。 */
export function markGeometryShared(geometry: ThreeNamespace.BufferGeometry): void {
  sharedGeometries.add(geometry);
}

/**
 * 取 payload 的模型根对象（缓存命中时返回克隆，几何共享）。
 * 首次访问并行 parse；payload 更换时释放旧缓存（dispose 全部共享几何）。
 */
export async function getLotModelObjects(
  payload: LotModelPayload,
): Promise<ThreeNamespace.Object3D[]> {
  if (cachedPayload !== payload) {
    releaseLotModelCache();
    const roots = await parseLotModelObjects(payload.glbs);
    for (const root of roots) {
      root.traverse((child) => {
        const mesh = child as ThreeNamespace.Mesh;
        if (mesh.isMesh && mesh.geometry) sharedGeometries.add(mesh.geometry);
      });
    }
    cachedPayload = payload;
    cachedRoots = roots;
  }
  return cachedRoots.map((root) => root.clone());
}

/** 释放模型缓存（payload 更换/视口销毁）：解除共享标记并 dispose。 */
export function releaseLotModelCache(): void {
  for (const root of cachedRoots) {
    root.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (mesh.isMesh && mesh.geometry) sharedGeometries.delete(mesh.geometry);
    });
  }
  for (const root of cachedRoots) {
    // 递归 dispose 几何/材质（材质在此前 rebuild 已被清场 dispose 过，
    // 重复 dispose 是幂等的）。
    root.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (mesh.isMesh) {
        mesh.geometry?.dispose();
        const material = mesh.material;
        if (Array.isArray(material)) material.forEach((item) => item.dispose());
        else material?.dispose();
      }
    });
  }
  cachedRoots = [];
  cachedPayload = null;
}
