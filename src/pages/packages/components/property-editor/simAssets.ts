import type * as ThreeNamespace from "three";
import type { LotModelPayload } from "@/api/tauri";
import { tauriApi } from "@/api";
import {
  markGeometryShared,
  parseLotModelContainer,
  parseLotModelObjects,
  pngBlobUrl,
} from "@/lib/three-gltf";
import { unitId } from "./unitGizmos";
import type { LotUnitDto } from "@/api/tauri";

/**
 * 真小人渲染的全局资产通道（props 文档 §3.1，2026-10-06）。
 *
 * 小人 = 组装模型：T-pose 身体（3 型）+ 头部（20 型，挂 Male_Head 颈点）
 * + outfit 调色板 tint。模型 key 来自小人 impostor class 22706EFA 的
 * 29 part 配置表，后端 read_sim_parts 提取为 SIMF 容器：
 * `SIMF | u32 ver=1 | u32 count | {u32 instance | u8 kind | u32 len | LOTM}`。
 * 全局资产（非 lot 资产）——进程级缓存，跨 lot 复用。
 *
 * 纹理槽位实测（§3.1）：小人部件 tex[0]=法线（蓝紫）、tex[1]=漫反射
 * （脸/手套/衣服 UV 清晰可见）——与 LOTM Raw 通道映射（[0]→slot0、
 * [1]→normal）相反，漫反射取 normalPng。UV 语义同树（RW4 v0 底部）→
 * flipY=true。
 */

/** SIMF 容器解析结果：单部件 LOTM 载荷 + 部件类别。 */
export interface SimPart {
  instance: number;
  kind: "body" | "head";
  payload: LotModelPayload;
}

export function parseSimPartsContainer(buffer: ArrayBuffer): SimPart[] {
  const view = new DataView(buffer);
  if (view.getUint32(0, true) !== 0x464d_4953) {
    throw new Error("sim parts: bad magic");
  }
  const count = view.getUint32(8, true);
  let pos = 12;
  const parts: SimPart[] = [];
  for (let i = 0; i < count; i += 1) {
    const instance = view.getUint32(pos, true);
    const kind: "body" | "head" = view.getUint8(pos + 4) === 0 ? "body" : "head";
    const len = view.getUint32(pos + 5, true);
    const lotm = buffer.slice(pos + 9, pos + 9 + len);
    pos += 9 + len;
    parts.push({ instance, kind, payload: parseLotModelContainer(lotm) });
  }
  return parts;
}

/** 进程级缓存：全局资产只拉一次；失败静默为空数组（spawner 走占位人形）。 */
let simPartsCache: Promise<SimPart[]> | null = null;
export function loadSimParts(): Promise<SimPart[]> {
  simPartsCache ??= tauriApi.packages.readSimParts()
    .then(parseSimPartsContainer)
    .catch(() => [] as SimPart[]);
  return simPartsCache;
}

interface SimPartTemplate {
  geometry: ThreeNamespace.BufferGeometry;
  map: ThreeNamespace.Texture | null;
  /** 模型空间 bbox（Z-up）：头部挂点对位用。 */
  box: { min: [number, number, number]; max: [number, number, number] };
}

const simTemplateCache = new Map<number, SimPartTemplate>();
const simTextureCache = new WeakMap<Uint8Array<ArrayBuffer>, ThreeNamespace.Texture>();

async function simPartTemplate(
  THREE: typeof ThreeNamespace,
  part: SimPart,
): Promise<SimPartTemplate | null> {
  const cached = simTemplateCache.get(part.instance);
  if (cached) return cached;
  let roots: ThreeNamespace.Object3D[];
  try {
    roots = await parseLotModelObjects(part.payload.glbs);
  } catch {
    return null;
  }
  let geometry: ThreeNamespace.BufferGeometry | null = null;
  for (const root of roots) {
    root.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (!mesh.isMesh || geometry || !mesh.geometry) return;
      geometry = mesh.geometry;
    });
    if (geometry) break;
  }
  if (!geometry) return null;
  const shared = geometry as ThreeNamespace.BufferGeometry;
  shared.computeBoundingBox();
  const bb = shared.boundingBox;
  if (!bb) return null;
  for (const root of roots) {
    root.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (mesh.isMesh && mesh.geometry) markGeometryShared(mesh.geometry);
    });
  }
  // 漫反射 = 文件级 tex[1]（LOTM normalPng 通道）；tex[0] 是法线图。
  const mapBytes =
    part.payload.materials?.[0]?.normalPng ??
    part.payload.materials?.[0]?.slot0Png ??
    null;
  let map = mapBytes ? simTextureCache.get(mapBytes) : undefined;
  if (mapBytes && !map) {
    try {
      const loaded = await new THREE.TextureLoader().loadAsync(pngBlobUrl(mapBytes));
      loaded.colorSpace = THREE.SRGBColorSpace;
      loaded.flipY = true;
      loaded.anisotropy = 4;
      map = loaded;
      simTextureCache.set(mapBytes, loaded);
    } catch {
      map = undefined;
    }
  }
  const template: SimPartTemplate = {
    geometry: shared,
    map: map ?? null,
    box: {
      min: [bb.min.x, bb.min.y, bb.min.z],
      max: [bb.max.x, bb.max.y, bb.max.z],
    },
  };
  simTemplateCache.set(part.instance, template);
  return template;
}

/** unit id → 数字 seed（FNV-1）：同一刷新点恒同一小人。 */
function simSeed(unit: LotUnitDto): number {
  const id = unitId(unit);
  let hash = 0x811c9dc5;
  for (let i = 0; i < id.length; i += 1) {
    hash ^= id.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = Math.imul(t ^ (t >>> 7), 61 | t) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * 组装一个小人：按 seed 随机 身体（3 型）+ 头（20 型），outfit 调色板以
 * 衣服色相 tint 近似（引擎真值是调色板纹理换列，本地未定位）。头部挂
 * 身体颈点（body maxZ 与 head minZ 对齐，留 0.04 重叠）。
 * 返回 null = 资产缺席（调用方退占位人形）。
 */
export async function getSimFigure(
  THREE: typeof ThreeNamespace,
  parts: SimPart[],
  unit: LotUnitDto,
): Promise<ThreeNamespace.Object3D | null> {
  const bodies = parts.filter((part) => part.kind === "body");
  const heads = parts.filter((part) => part.kind === "head");
  if (!bodies.length || !heads.length) return null;
  const rand = mulberry32(simSeed(unit));
  const body = bodies[Math.floor(rand() * bodies.length) % bodies.length];
  const head = heads[Math.floor(rand() * heads.length) % heads.length];
  const [bodyTemplate, headTemplate] = await Promise.all([
    simPartTemplate(THREE, body),
    simPartTemplate(THREE, head),
  ]);
  if (!bodyTemplate || !headTemplate) return null;

  // outfit 调色板近似：淡彩 tint（明度抬高避免脏色）
  const tint = new THREE.Color().setHSL(rand(), 0.35, 0.72);
  const bodyMesh = new THREE.Mesh(
    bodyTemplate.geometry,
    new THREE.MeshStandardMaterial({
      map: bodyTemplate.map,
      color: bodyTemplate.map ? tint : 0xffffff,
      roughness: 0.75,
      metalness: 0,
      side: THREE.DoubleSide,
    }),
  );
  const headMesh = new THREE.Mesh(
    headTemplate.geometry,
    new THREE.MeshStandardMaterial({
      map: headTemplate.map,
      roughness: 0.75,
      metalness: 0,
      side: THREE.DoubleSide,
    }),
  );
  // 颈点对位：头顶到身体 top（留 0.04 嵌入）；XY 按身体中轴
  const bodyCX = (bodyTemplate.box.min[0] + bodyTemplate.box.max[0]) / 2;
  const bodyCY = (bodyTemplate.box.min[1] + bodyTemplate.box.max[1]) / 2;
  const headCX = (headTemplate.box.min[0] + headTemplate.box.max[0]) / 2;
  const headCY = (headTemplate.box.min[1] + headTemplate.box.max[1]) / 2;
  headMesh.position.set(
    bodyCX - headCX,
    bodyCY - headCY,
    bodyTemplate.box.max[2] - headTemplate.box.min[2] - 0.04,
  );
  const group = new THREE.Group();
  group.add(bodyMesh, headMesh);
  // 叶卡口径同树：不做投影（alpha 不进深度 pass）
  bodyMesh.receiveShadow = true;
  headMesh.receiveShadow = true;
  return group;
}
