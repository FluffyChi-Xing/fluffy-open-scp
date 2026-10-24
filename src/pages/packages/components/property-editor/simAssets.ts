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
 * 游戏 Sim shader 的 `sourceTex` 不是普通漫反射图：R/G/A 分别承载 AO、
 * palette 行和剪影透明度，最终颜色由全局 body/clothes palette 查表得到。
 * SIMF 的 slot0 保留这张 sourceTex；只有缺少它时才回退到普通 diffuse。
 * UV 语义同树（RW4 v0 底部）→ flipY=true。
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
    const kind: "body" | "head" =
      view.getUint8(pos + 4) === 0 ? "body" : "head";
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
  simPartsCache ??= tauriApi.packages
    .readSimParts()
    .then(parseSimPartsContainer)
    .catch(() => [] as SimPart[]);
  return simPartsCache;
}

interface SimPartTemplate {
  geometry: ThreeNamespace.BufferGeometry;
  map: ThreeNamespace.Texture | null;
  mapKind: "palette" | "diffuse";
  /** 模型空间 bbox（Z-up）：头部挂点对位用。 */
  box: { min: [number, number, number]; max: [number, number, number] };
}

const simTemplateCache = new Map<number, SimPartTemplate>();
const simTextureCache = new WeakMap<
  Uint8Array<ArrayBuffer>,
  ThreeNamespace.Texture
>();

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
  // slot0 是 Sim sourceTex（R=AO、G=paletteV、A=alpha），不能再按文件大小
  // 在法线/漫反射之间猜图。只有老版本载荷没有 slot0 时才使用普通 diffuse；
  // normalPng 仅作为最后的兼容回退，避免空白预览。
  const material = part.payload.materials?.[0];
  const mapChoice = material?.slot0Png
    ? { bytes: material.slot0Png, mapKind: "palette" as const }
    : material?.baseColorPng
      ? { bytes: material.baseColorPng, mapKind: "diffuse" as const }
      : material?.normalPng
        ? { bytes: material.normalPng, mapKind: "diffuse" as const }
        : null;
  const mapBytes = mapChoice?.bytes ?? null;
  let map = mapBytes ? simTextureCache.get(mapBytes) : undefined;
  if (mapBytes && !map) {
    try {
      const loaded = await new THREE.TextureLoader().loadAsync(
        pngBlobUrl(mapBytes),
      );
      loaded.colorSpace =
        mapChoice?.mapKind === "palette"
          ? THREE.NoColorSpace
          : THREE.SRGBColorSpace;
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
    mapKind: mapChoice?.mapKind ?? "diffuse",
    box: {
      min: [bb.min.x, bb.min.y, bb.min.z],
      max: [bb.max.x, bb.max.y, bb.max.z],
    },
  };
  simTemplateCache.set(part.instance, template);
  return template;
}

/** unit id → 数字 seed（FNV-1）：同一 UnitLocation 的预览结果稳定。 */
function simSeed(unit: LotUnitDto): number {
  const id = unitId(unit);
  let hash = 0x811c9dc5;
  for (let i = 0; i < id.length; i += 1) {
    hash ^= id.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

/**
 * Property Editor 的人口预览只表达“这个锚点可能暂时有几名 Agent”，不
 * 伪造游戏运行时的精确人数。游戏里的 UnitLocation 只是命名锚点，是否
 * 激活、由哪条 ER2/Transport 规则创建 Agent，以及 Agent 是否组队，都在
 * 模拟运行时决定。这里用稳定 seed 做一次可复现的蒙特卡洛样本：未激活
 * 的点交给调用方显示锚点，激活的点显示 2~5 名外观各异的示意 Agent。
 */
export interface SpawnerPopulationPreview {
  seed: number;
  count: number;
  /** 仅供 UI/渲染标记使用，不代表游戏中的 spawn chance 字段。 */
  active: boolean;
}

const SPAWNER_PREVIEW_ACTIVE_CHANCE = 0.62;
const SPAWNER_PREVIEW_MIN_COUNT = 2;
const SPAWNER_PREVIEW_COUNT_RANGE = 4;

export function getSpawnerPopulationPreview(
  unit: LotUnitDto,
): SpawnerPopulationPreview {
  const seed = simSeed(unit) ^ 0x5f37_59df;
  const rand = mulberry32(seed);
  if (rand() >= SPAWNER_PREVIEW_ACTIVE_CHANCE) {
    return { seed, count: 0, active: false };
  }
  const count =
    SPAWNER_PREVIEW_MIN_COUNT +
    Math.floor(rand() * SPAWNER_PREVIEW_COUNT_RANGE);
  return { seed, count, active: true };
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

type SimRgb = readonly [number, number, number];

// 游戏通过全局 palette 纹理决定肤色和服装色。编辑器暂时没有随 SIMF
// 下发这两张全局纹理，因此保留相同的“两列查色”结构，用线性 RGB 色板
// 做本地近似；颜色选择由 UnitLocation+成员序号稳定决定。
const SIM_SKIN_PALETTE: readonly SimRgb[] = [
  [0.92, 0.68, 0.49],
  [0.72, 0.43, 0.27],
  [0.48, 0.25, 0.14],
  [0.97, 0.78, 0.61],
];
const SIM_CLOTHES_PALETTE: readonly SimRgb[] = [
  [0.12, 0.27, 0.72],
  [0.12, 0.49, 0.28],
  [0.42, 0.18, 0.62],
  [0.48, 0.23, 0.1],
  [0.08, 0.18, 0.36],
  [0.62, 0.12, 0.16],
  [0.46, 0.46, 0.48],
  [0.78, 0.58, 0.1],
];

function simMaterial(
  THREE: typeof ThreeNamespace,
  template: SimPartTemplate,
  skin: SimRgb,
  clothes: SimRgb,
): ThreeNamespace.MeshStandardMaterial {
  const material = new THREE.MeshStandardMaterial({
    map: template.map,
    roughness: 0.75,
    metalness: 0,
    alphaTest: template.mapKind === "palette" ? 0.3 : 0,
    side: THREE.DoubleSide,
  });
  if (template.mapKind !== "palette") return material;

  material.onBeforeCompile = (shader) => {
    shader.uniforms.simBodyColor = { value: new THREE.Color(...skin) };
    shader.uniforms.simClothesColor = { value: new THREE.Color(...clothes) };
    shader.fragmentShader = shader.fragmentShader.replace(
      "#include <map_pars_fragment>",
      `#include <map_pars_fragment>
      uniform vec3 simBodyColor;
      uniform vec3 simClothesColor;
      `,
    );
    shader.fragmentShader = shader.fragmentShader.replace(
      "#include <map_fragment>",
      `#include <map_fragment>

      // Match simParticleLightPS: sourceTex.r=AO, .g=paletteV, .a=alpha.
      // clothesPaletteTex.a is the game's clothing mask; its global palette is
      // not part of the per-part SIMF payload, so paletteV is a stable local
      // approximation that keeps skin and clothes from receiving one tint.
      vec4 simSource = texture2D( map, vMapUv );
      float simAo = max(simSource.r, 0.28);
      float simClothesMask = smoothstep(0.36, 0.64, simSource.g);
      vec3 simTint = mix(simBodyColor, simClothesColor, simClothesMask);
      diffuseColor.rgb = simTint * simAo;
      diffuseColor.a *= simSource.a;
      `,
    );
  };
  return material;
}

/**
 * 组装一个小人：按 seed 随机 身体（3 型）+ 头（20 型），并选择稳定的
 * 肤色/服装 palette 列。头部挂
 * 身体颈点（body maxZ 与 head minZ 对齐，留 0.04 重叠）。
 * `seedSalt` 用于同一 UnitLocation 的多 Agent 预览，确保每个成员的
 * body/head 组合稳定且互不相同。返回 null = 资产缺席。
 */
export async function getSimFigure(
  THREE: typeof ThreeNamespace,
  parts: SimPart[],
  unit: LotUnitDto,
  seedSalt = 0,
): Promise<ThreeNamespace.Object3D | null> {
  const bodies = parts.filter((part) => part.kind === "body");
  const heads = parts.filter((part) => part.kind === "head");
  if (!bodies.length || !heads.length) return null;
  const saltedSeed = simSeed(unit) ^ Math.imul(seedSalt + 1, 0x9e37_79b9);
  const rand = mulberry32(saltedSeed >>> 0);
  const body = bodies[Math.floor(rand() * bodies.length) % bodies.length];
  const head = heads[Math.floor(rand() * heads.length) % heads.length];
  const [bodyTemplate, headTemplate] = await Promise.all([
    simPartTemplate(THREE, body),
    simPartTemplate(THREE, head),
  ]);
  if (!bodyTemplate || !headTemplate) return null;

  const skin = SIM_SKIN_PALETTE[Math.floor(rand() * SIM_SKIN_PALETTE.length)];
  const clothes =
    SIM_CLOTHES_PALETTE[Math.floor(rand() * SIM_CLOTHES_PALETTE.length)];
  const bodyMesh = new THREE.Mesh(
    bodyTemplate.geometry,
    simMaterial(THREE, bodyTemplate, skin, clothes),
  );
  const headMesh = new THREE.Mesh(
    headTemplate.geometry,
    simMaterial(THREE, headTemplate, skin, clothes),
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

/**
 * 构造一个 UnitLocation 的人群示意。根节点不带 Unit transform，调用方
 * 负责把整个群体放到建筑的局部锚点；Sim 部件的模型空间是 Z-up（头颈
 * 对齐也使用 bbox 的 Z 轴），所以成员必须在局部 XY 地面平面散开并绕 Z
 * 轴转向。这样不会把“朝向随机”误变成绕水平轴翻倒。
 */
export async function getSimPopulation(
  THREE: typeof ThreeNamespace,
  parts: SimPart[],
  unit: LotUnitDto,
): Promise<ThreeNamespace.Group | null> {
  const preview = getSpawnerPopulationPreview(unit);
  if (!preview.active || preview.count <= 0) return null;

  const group = new THREE.Group();
  const rand = mulberry32(preview.seed ^ 0x243f_6a88);
  let rendered = 0;
  for (let index = 0; index < preview.count; index += 1) {
    const figure = await getSimFigure(THREE, parts, unit, index);
    if (!figure) continue;
    const angle = rand() * Math.PI * 2;
    const radius = 0.28 + rand() * 0.58;
    figure.position.set(Math.cos(angle) * radius, Math.sin(angle) * radius, 0);
    figure.rotation.z = angle + Math.PI + (rand() - 0.5) * 0.7;
    figure.userData.populationMember = index;
    group.add(figure);
    rendered += 1;
  }
  if (!rendered) return null;
  group.userData.previewPopulation = rendered;
  group.userData.previewPopulationMode = "runtime-sample";
  return group;
}
