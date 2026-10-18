import * as THREE from "three";
import type { LotMaterialTextures, LotModelPayload } from "@/api/tauri";
import {
  MATERIAL_SLOTS,
  type BuildingPart,
  type MaterialSlotId,
} from "./schema";

/** 盒体部件 → 合成 LotModelPayload。
 *
 * 链路：部件按材质槽分组 → 每槽合并一份 BufferGeometry → GLTFExporter
 * 逐槽导出 GLB → 以 meshUvKinds=1 + materials[i].baseColorPng 注入，
 * Property Editor 视口的精渲染链（getDeferredMaps →
 * applyDeferredMaterialMaps）原样消费。
 *
 * 【UV 口径 = 游戏贴图映射】每个面整张贴图一次（UV 0..1 随面拉伸），
 * 不做逐格平铺：游戏建筑就是「整张立面图 ↔ 整面墙」的 atlas 语义，
 * 地块地面纹理同样恰好铺满足迹一次。带窗立面图绑到 wall 槽即得
 * 游戏观感；小块装饰件（壁柱/线脚）各面独立整贴，不会串到邻面。
 *
 * 坐标系：生成器直接以 Z-up 建模；parseLotModelObjects 会剥掉 GLB 根
 * 旋转、视口 world 组本身就是 Z-up 世界，因此导出时不做任何旋转。
 */

interface MeshArrays {
  position: number[];
  normal: number[];
  uv: number[];
  color: number[];
  index: number[];
}

function newArrays(): MeshArrays {
  return { position: [], normal: [], uv: [], color: [], index: [] };
}

type ColorRGB = [number, number, number];

/** sRGB hex → 线性 RGB（glTF COLOR_0 按线性解释）。 */
export function hexToLinear(hex: string): ColorRGB {
  const value = Number.parseInt(hex.replace("#", ""), 16);
  const channel = (shift: number) => {
    const srgb = ((value >> shift) & 0xff) / 255;
    return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
  };
  return [channel(16), channel(8), channel(0)];
}

/** 追加一个四边形（独立顶点，UV 全幅 0..1）。u×v 必须等于 normal 方向。 */
function appendQuad(
  target: MeshArrays,
  origin: THREE.Vector3,
  u: THREE.Vector3,
  v: THREE.Vector3,
  normal: THREE.Vector3,
  color: ColorRGB,
) {
  const base = target.position.length / 3;
  const corners: [number, number][] = [
    [0, 0],
    [1, 0],
    [1, 1],
    [0, 1],
  ];
  for (const [cu, cv] of corners) {
    target.position.push(
      origin.x + u.x * cu + v.x * cv,
      origin.y + u.y * cu + v.y * cv,
      origin.z + u.z * cu + v.z * cv,
    );
    target.normal.push(normal.x, normal.y, normal.z);
    target.uv.push(cu, cv);
    target.color.push(color[0], color[1], color[2]);
  }
  target.index.push(base, base + 1, base + 2, base, base + 2, base + 3);
}

/** 单个盒体的 6 个面：每面一张贴图（UV 0..1 整幅拉伸）。 */
export function appendBox(
  target: MeshArrays,
  center: readonly [number, number, number],
  size: readonly [number, number, number],
  color: ColorRGB,
) {
  const [cx, cy, cz] = center;
  const [sx, sy, sz] = size.map((value) => Math.max(0.01, value)) as [
    number,
    number,
    number,
  ];
  const half = new THREE.Vector3(sx / 2, sy / 2, sz / 2);
  // u/v 是带长度的边向量（顶点 = origin + u·cu + v·cv），法线单独传单位向量
  const faces: {
    normal: THREE.Vector3;
    u: THREE.Vector3;
    v: THREE.Vector3;
    origin: THREE.Vector3;
  }[] = [
    // +X：u=+Y·sy, v=+Z·sz
    { normal: new THREE.Vector3(1, 0, 0), u: new THREE.Vector3(0, sy, 0), v: new THREE.Vector3(0, 0, sz), origin: new THREE.Vector3(cx + half.x, cy - half.y, cz - half.z) },
    // -X：u=+Z·sz, v=+Y·sy
    { normal: new THREE.Vector3(-1, 0, 0), u: new THREE.Vector3(0, 0, sz), v: new THREE.Vector3(0, sy, 0), origin: new THREE.Vector3(cx - half.x, cy - half.y, cz - half.z) },
    // +Y：u=+Z·sz, v=+X·sx
    { normal: new THREE.Vector3(0, 1, 0), u: new THREE.Vector3(0, 0, sz), v: new THREE.Vector3(sx, 0, 0), origin: new THREE.Vector3(cx - half.x, cy + half.y, cz - half.z) },
    // -Y：u=+X·sx, v=+Z·sz
    { normal: new THREE.Vector3(0, -1, 0), u: new THREE.Vector3(sx, 0, 0), v: new THREE.Vector3(0, 0, sz), origin: new THREE.Vector3(cx - half.x, cy - half.y, cz - half.z) },
    // +Z：u=+X·sx, v=+Y·sy
    { normal: new THREE.Vector3(0, 0, 1), u: new THREE.Vector3(sx, 0, 0), v: new THREE.Vector3(0, sy, 0), origin: new THREE.Vector3(cx - half.x, cy - half.y, cz + half.z) },
    // -Z：u=+Y·sy, v=+X·sx
    { normal: new THREE.Vector3(0, 0, -1), u: new THREE.Vector3(0, sy, 0), v: new THREE.Vector3(sx, 0, 0), origin: new THREE.Vector3(cx - half.x, cy - half.y, cz - half.z) },
  ];
  for (const face of faces) {
    appendQuad(target, face.origin, face.u, face.v, face.normal, color);
  }
}

function arraysToGeometry(arrays: MeshArrays): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute(
    "position",
    new THREE.Float32BufferAttribute(arrays.position, 3),
  );
  geometry.setAttribute(
    "normal",
    new THREE.Float32BufferAttribute(arrays.normal, 3),
  );
  geometry.setAttribute("uv", new THREE.Float32BufferAttribute(arrays.uv, 2));
  geometry.setAttribute(
    "color",
    new THREE.Float32BufferAttribute(arrays.color, 3),
  );
  geometry.setIndex(arrays.index);
  return geometry;
}

/** 槽 → 缺省顶点色（绑定贴图的槽用白色，避免给贴图二次染色）。 */
export interface SlotColorResolver {
  (slot: MaterialSlotId, textured: boolean): ColorRGB;
}

export function defaultSlotColor(slot: MaterialSlotId, textured: boolean): ColorRGB {
  if (textured) return [1, 1, 1];
  const definition = MATERIAL_SLOTS.find((entry) => entry.id === slot);
  return hexToLinear(definition?.defaultColor ?? "#cccccc");
}

/** 槽 → BufferGeometry（纯数据，测试直接消费）。 */
export function buildSlotGeometries(
  parts: readonly BuildingPart[],
  resolveColor: SlotColorResolver = defaultSlotColor,
  texturedSlots: ReadonlySet<MaterialSlotId> = new Set(),
): Map<MaterialSlotId, THREE.BufferGeometry> {
  const arrays = new Map<MaterialSlotId, MeshArrays>();
  for (const piece of parts) {
    let target = arrays.get(piece.slot);
    if (!target) {
      target = newArrays();
      arrays.set(piece.slot, target);
    }
    appendBox(
      target,
      piece.center,
      piece.size,
      resolveColor(piece.slot, texturedSlots.has(piece.slot)),
    );
  }
  const geometries = new Map<MaterialSlotId, THREE.BufferGeometry>();
  for (const [slot, data] of arrays) geometries.set(slot, arraysToGeometry(data));
  return geometries;
}

export interface BuildPayloadOptions {
  parts: readonly BuildingPart[];
  /** 槽 → PNG 字节（用户绑定的游戏 raster 贴图）。 */
  textures: ReadonlyMap<MaterialSlotId, Uint8Array<ArrayBuffer>>;
  /** 诊断文本里的来源说明（槽 → 描述）。 */
  textureLabels?: ReadonlyMap<MaterialSlotId, string>;
}

/** 部件 + 贴图绑定 → 合成 LotModelPayload（每槽一个 GLB mesh）。 */
export async function buildModelPayload(
  options: BuildPayloadOptions,
): Promise<LotModelPayload> {
  const { GLTFExporter } = await import(
    "three/examples/jsm/exporters/GLTFExporter.js"
  );
  const texturedSlots = new Set(options.textures.keys());
  const geometries = buildSlotGeometries(
    options.parts,
    defaultSlotColor,
    texturedSlots,
  );
  // 固定槽序（MATERIAL_SLOTS 顺序），只导出有几何的槽
  const slots = MATERIAL_SLOTS.filter((slot) => geometries.has(slot.id));
  const exporter = new GLTFExporter();
  const glbs: ArrayBuffer[] = [];
  const materials: LotMaterialTextures[] = [];
  const diagnostics: string[] = [];
  for (const slot of slots) {
    const mesh = new THREE.Mesh(geometries.get(slot.id)!);
    mesh.name = `procedural-${slot.id}`;
    const glb = await new Promise<ArrayBuffer>((resolve, reject) => {
      exporter.parse(
        mesh,
        result => resolve(result as ArrayBuffer),
        error => reject(error instanceof Error ? error : new Error(String(error))),
        { binary: true },
      );
    });
    glbs.push(glb);
    const png = options.textures.get(slot.id) ?? null;
    materials.push({
      baseColorPng: png,
      slot0Png: null,
      normalPng: null,
      roughnessPng: null,
      aoPng: null,
      tintPng: null,
      palettePng: null,
      shaderPng: null,
      interiorPng: null,
      reliefPng: null,
      paramsF32: null,
      paramCols: 0,
    });
    diagnostics.push(
      `${slot.id}: ${options.textureLabels?.get(slot.id) ?? (png ? "texture" : "default color")}`,
    );
    mesh.geometry.dispose();
  }
  return {
    glbs,
    materials,
    meshMaterialIndices: slots.map((_, index) => index),
    meshUvKinds: slots.map(() => 1),
    diagnostics: `procedural facade-kit · ${diagnostics.join(" · ")}`,
  };
}
