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
  if (version !== 8 && version !== 9 && version !== 10) {
    throw new Error(`unsupported lot model payload version ${version}`);
  }
  // v8=9 张（含已废弃 relief）、v9=8 张、v10=9 张（第 9 张 = slot0 原始
  // 纹理，车辆/prop 漫反射；建筑槽位语义不同，仅 prop 消费）
  const pngsPerMaterial = version >= 10 ? 9 : version >= 9 ? 8 : 9;
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
    const [
      baseColorPng,
      normalPng,
      roughnessPng,
      aoPng,
      tintPng,
      palettePng,
      shaderPng,
      interiorPng,
      slot0Png,
    ] = pngs;
    // v8 的第 9 张（relief）读取后即弃：字段仅为类型兼容保留，无消费方。
    // v10 的第 9 张 = slot0 原始纹理（车辆 diffuse，见 LotMaterialTextures）。
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
      slot0Png: slot0Png ?? null,
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
 *
 * 【2026-10-08】GLB 解析 Worker 实验回滚：EP1 建筑贴图乱码与 Worker 路径
 * 稳定复现相关（用户 A/B 实证：Worker=1 乱/Worker=2 好；transfer/克隆两种
 * 传输均乱；重建串行化修复无效），根因在 Worker 环境的解析行为差异，
 * 未定谳前回归主线程。取证基建保留：sc-exporter example dump_facade_glbs
 * （真实 EP1 facade GLB 落盘）+ git 历史（aec08b2..e1c276b 可查实现）。
 */
export async function parseLotModelObjects(
  glbs: ArrayBuffer[],
  baseRotationX = 0,
): Promise<ThreeNamespace.Object3D[]> {
  const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
  const loader = new GLTFLoader();
  const objects = await Promise.all(
    glbs.map(async (glb) => {
      const gltf = await loader.parseAsync(glb, "");
      for (const child of gltf.scene.children) {
        child.rotation.set(baseRotationX, 0, 0);
      }
      return gltf.scene;
    }),
  );
  return objects;
}

/**
 * 外部 GLB 的结构与上轴检测（用户指令 2026-10-25：Blender 导入与游戏导入
 * 的模型轴向不同，自动转换；同时为白模诊断提供材质/贴图事实）。
 *
 * 上轴三分判定（按观测到的三种导出口径）：
 *  1. 场景根节点带 ±90°X 旋转 → 数据为 Z-up（游戏 gltf.rs 口径：Z-up 数据
 *     + 根旋转拼成 Y-up 文件），渲染剥掉根旋转即可；
 *  2. 根节点无旋转且 POSITION 包围盒 Z 跨度 > Y 跨度 → 数据为 Z-up
 *     （blender_asset_pipeline 契约导出 export_yup=False，Z-up 直出），
 *     渲染原样入组；
 *  3. 其余 → glTF 规范 Y-up（Blender 默认 export_yup=True），渲染 +90°X
 *     搬到 Z-up 世界。
 * 只读 GLB 容器开头的 JSON chunk，不解码网格。
 */
export interface GlbInspection {
  upAxis: "y" | "z";
  /** 判定依据（诊断面板展示）。 */
  upAxisBasis: string;
  meshCount: number;
  materialCount: number;
  /** 内嵌贴图数（0 = 契约 GLB，渲染需走 COLOR_0/白模兜底）。 */
  imageCount: number;
  /** 任一 primitive 带 COLOR_0（探针契约 GLB 的调色板顶点色烘焙）。 */
  hasVertexColors: boolean;
  /** TEXCOORD_N 集合上界（0 = 无 UV）。 */
  texcoordSets: number;
}

export function inspectGlb(bytes: ArrayBuffer): GlbInspection {
  const fallback: GlbInspection = {
    upAxis: "y",
    upAxisBasis: "默认",
    meshCount: 0,
    materialCount: 0,
    imageCount: 0,
    hasVertexColors: false,
    texcoordSets: 0,
  };
  try {
    const view = new DataView(bytes);
    if (bytes.byteLength < 20 || view.getUint32(0, true) !== 0x46546c67)
      return fallback; // 非 GLB 容器（容错：按 glTF 规范 Y-up 处理）
    if (view.getUint32(16, true) !== 0x4e4f534a) return fallback; // chunk 非 JSON
    const jsonLength = view.getUint32(12, true);
    const text = new TextDecoder().decode(
      new Uint8Array(bytes, 20, Math.min(jsonLength, bytes.byteLength - 20)),
    );
    const json = JSON.parse(text) as {
      scene?: number;
      scenes?: { nodes?: number[] }[];
      nodes?: { rotation?: number[]; mesh?: number }[];
      meshes?: { primitives?: { attributes?: Record<string, number> }[] }[];
      accessors?: { min?: number[]; max?: number[] }[];
      materials?: unknown[];
      images?: unknown[];
    };
    const scene = json.scenes?.[json.scene ?? 0];
    const nodes = json.nodes ?? [];
    // 判定 1：根节点 ±90°X 旋转（游戏 gltf.rs 口径）
    let rotatedRoot = false;
    for (const index of scene?.nodes ?? []) {
      const rotation = nodes[index]?.rotation;
      if (Array.isArray(rotation) && Math.abs(rotation[0]) > 0.7) {
        rotatedRoot = true;
        break;
      }
    }
    // POSITION 包围盒（判定 2 的 Z/Y 跨度比较；法线等其它 VEC3 不参与）
    const positionAccessors = new Set<number>();
    for (const mesh of json.meshes ?? []) {
      for (const primitive of mesh.primitives ?? []) {
        const position = primitive.attributes?.POSITION;
        if (typeof position === "number") positionAccessors.add(position);
      }
    }
    let minX = Infinity, minY = Infinity, minZ = Infinity;
    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;
    for (const index of positionAccessors) {
      const accessor = json.accessors?.[index];
      if (!accessor?.min || !accessor?.max) continue;
      minX = Math.min(minX, accessor.min[0]); maxX = Math.max(maxX, accessor.max[0]);
      minY = Math.min(minY, accessor.min[1]); maxY = Math.max(maxY, accessor.max[1]);
      minZ = Math.min(minZ, accessor.min[2]); maxZ = Math.max(maxZ, accessor.max[2]);
    }
    let upAxis: "y" | "z";
    let upAxisBasis: string;
    if (rotatedRoot) {
      upAxis = "z";
      upAxisBasis = "根节点 ±90°X 旋转（游戏导出口径，数据 Z-up）";
    } else if (
      Number.isFinite(minZ) &&
      maxZ - minZ > maxY - minY
    ) {
      upAxis = "z";
      upAxisBasis = "根节点无旋转且 Z 跨度最大（契约导出 export_yup=False，数据 Z-up）";
    } else {
      upAxis = "y";
      upAxisBasis = "glTF 规范 Y-up（Blender 默认导出）";
    }
    let hasVertexColors = false;
    const texcoords = new Set<number>();
    for (const mesh of json.meshes ?? []) {
      for (const primitive of mesh.primitives ?? []) {
        for (const [key, value] of Object.entries(primitive.attributes ?? {})) {
          if (key === "COLOR_0" && value !== undefined) hasVertexColors = true;
          const uv = /^TEXCOORD_(\d+)$/.exec(key);
          if (uv) texcoords.add(Number(uv[1]));
        }
      }
    }
    return {
      upAxis,
      upAxisBasis,
      meshCount: (json.meshes ?? []).length,
      materialCount: (json.materials ?? []).length,
      imageCount: (json.images ?? []).length,
      hasVertexColors,
      texcoordSets: texcoords.size ? Math.max(...texcoords) + 1 : 0,
    };
  } catch {
    return fallback;
  }
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
    // 外部导入 GLB（Blender 管线）按 glTF 规范 Y-up；原生 LOTM 网格已
    // 在后端烘焙为 Z-up，根旋转剥零。upAxis 驱动统一到视口 Z-up 世界。
    const roots = await parseLotModelObjects(
      payload.glbs,
      payload.upAxis === "y" ? Math.PI / 2 : 0,
    );
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
