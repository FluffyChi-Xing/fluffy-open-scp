import * as THREE from "three";
import { spacedFacadeFaces } from "./windowSpacing";
import { buildMassFaces, type SolidFace } from "./solidGeometry";
import type { LotMaterialTextures, LotModelPayload } from "@/api/tauri";
import {
  MATERIAL_SLOTS,
  type BuildingPart,
  type MaterialSlotId,
} from "./schema";

import type {
  BuildingMaterialSource,
  SurfaceMaterial,
} from "./propertyMaterial";

/** Z-up procedural boxes. Source building4 materials retain their parameter
 * tables and three UV channels; an unbound model falls back to vertex colours. */
interface MeshArrays {
  position: number[];
  normal: number[];
  uv: number[];
  color: number[];
  index: number[];
  uv1?: number[];
  uv2?: number[];
  uv3?: number[];
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
  surface?: SurfaceMaterial,
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
    if (surface) {
      // Shared world projection keeps adjacent storeys aligned. Z is always up
      // on vertical faces (including -X/+Y, whose winding uses swapped u/v).
      const point = new THREE.Vector3(
        origin.x + u.x * cu + v.x * cv,
        origin.y + u.y * cu + v.y * cv,
        origin.z + u.z * cu + v.z * cv,
      );
      const horizontal =
        Math.abs(normal.z) > 0.7
          ? point.x
          : -normal.y * point.x + normal.x * point.y;
      // Roof atlas cells are authored in the same world-planar projection as
      // the game; use both plan axes. Vertical faces keep metres-up.
      const vertical = Math.abs(normal.z) > 0.7 ? point.y : point.z;
      (target.uv1 ??= []).push(surface.column / 255, surface.seed, 0, 0);
      const project = (mapping: SurfaceMaterial["base"]) =>
        mapping.offset.map(
          (offset, axis) =>
            offset +
            mapping.horizontal[axis] * horizontal +
            mapping.vertical[axis] * vertical,
        );
      (target.uv2 ??= []).push(...project(surface.base));
      (target.uv3 ??= []).push(...project(surface.top));
    }
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
  resolveSurface?: (normal: THREE.Vector3) => {
    target: MeshArrays;
    surface: SurfaceMaterial;
  },
  rotation = 0,
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
    {
      normal: new THREE.Vector3(1, 0, 0),
      u: new THREE.Vector3(0, sy, 0),
      v: new THREE.Vector3(0, 0, sz),
      origin: new THREE.Vector3(cx + half.x, cy - half.y, cz - half.z),
    },
    // -X：u=+Z·sz, v=+Y·sy
    {
      normal: new THREE.Vector3(-1, 0, 0),
      u: new THREE.Vector3(0, 0, sz),
      v: new THREE.Vector3(0, sy, 0),
      origin: new THREE.Vector3(cx - half.x, cy - half.y, cz - half.z),
    },
    // +Y：u=+Z·sz, v=+X·sx
    {
      normal: new THREE.Vector3(0, 1, 0),
      u: new THREE.Vector3(0, 0, sz),
      v: new THREE.Vector3(sx, 0, 0),
      origin: new THREE.Vector3(cx - half.x, cy + half.y, cz - half.z),
    },
    // -Y：u=+X·sx, v=+Z·sz
    {
      normal: new THREE.Vector3(0, -1, 0),
      u: new THREE.Vector3(sx, 0, 0),
      v: new THREE.Vector3(0, 0, sz),
      origin: new THREE.Vector3(cx - half.x, cy - half.y, cz - half.z),
    },
    // +Z：u=+X·sx, v=+Y·sy
    {
      normal: new THREE.Vector3(0, 0, 1),
      u: new THREE.Vector3(sx, 0, 0),
      v: new THREE.Vector3(0, sy, 0),
      origin: new THREE.Vector3(cx - half.x, cy - half.y, cz + half.z),
    },
    // -Z：u=+Y·sy, v=+X·sx
    {
      normal: new THREE.Vector3(0, 0, -1),
      u: new THREE.Vector3(0, sy, 0),
      v: new THREE.Vector3(sx, 0, 0),
      origin: new THREE.Vector3(cx - half.x, cy - half.y, cz - half.z),
    },
  ];
  for (const face of faces) {
    if (rotation) {
      const rotate = (value: THREE.Vector3) => {
        const x = value.x * Math.cos(rotation) - value.y * Math.sin(rotation);
        const y = value.x * Math.sin(rotation) + value.y * Math.cos(rotation);
        return new THREE.Vector3(x, y, value.z);
      };
      const centerVector = new THREE.Vector3(cx, cy, cz);
      const localOrigin = face.origin.clone().sub(centerVector);
      face.origin.copy(centerVector).add(rotate(localOrigin));
      face.u.copy(rotate(face.u));
      face.v.copy(rotate(face.v));
      face.normal.copy(rotate(face.normal)).normalize();
    }
    const binding = resolveSurface?.(face.normal);
    appendQuad(
      binding?.target ?? target,
      face.origin,
      face.u,
      face.v,
      face.normal,
      color,
      binding?.surface,
    );
  }
}

function appendSolidFace(
  target: MeshArrays,
  face: SolidFace,
  color: ColorRGB,
  surface?: SurfaceMaterial,
) {
  const start = target.position.length / 3;
  const normal = face.normal;
  for (const point of face.vertices) {
    target.position.push(point.x, point.y, point.z);
    target.normal.push(normal.x, normal.y, normal.z);
    const horizontal =
      Math.abs(normal.z) > 0.7
        ? point.x
        : -normal.y * point.x + normal.x * point.y;
    const vertical = Math.abs(normal.z) > 0.7 ? point.y : point.z;
    target.uv.push(horizontal, vertical);
    target.color.push(...color);
    if (surface) {
      (target.uv1 ??= []).push(surface.column / 255, surface.seed, 0, 0);
      const project = (mapping: SurfaceMaterial["base"]) =>
        mapping.offset.map(
          (offset, axis) =>
            offset +
            mapping.horizontal[axis] * horizontal +
            mapping.vertical[axis] * vertical,
        );
      (target.uv2 ??= []).push(...project(surface.base));
      (target.uv3 ??= []).push(...project(surface.top));
    }
  }
  for (let i = 1; i < face.vertices.length - 1; i++) {
    const cross = face.vertices[i]
      .clone()
      .sub(face.vertices[0])
      .cross(face.vertices[i + 1].clone().sub(face.vertices[0]));
    if (cross.lengthSq() > 1e-12)
      target.index.push(start, start + i, start + i + 1);
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
  if (arrays.uv1)
    geometry.setAttribute(
      "uv1",
      new THREE.Float32BufferAttribute(arrays.uv1, 4),
    );
  if (arrays.uv2)
    geometry.setAttribute(
      "uv2",
      new THREE.Float32BufferAttribute(arrays.uv2, 2),
    );
  if (arrays.uv3)
    geometry.setAttribute(
      "uv3",
      new THREE.Float32BufferAttribute(arrays.uv3, 2),
    );
  geometry.setIndex(arrays.index);
  return geometry;
}

/** 槽 → 缺省顶点色（绑定贴图的槽用白色，避免给贴图二次染色）。 */
export interface SlotColorResolver {
  (slot: MaterialSlotId, textured: boolean): ColorRGB;
}

export function defaultSlotColor(
  slot: MaterialSlotId,
  textured: boolean,
): ColorRGB {
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
  for (const face of buildMassFaces(parts)) {
    const slot: MaterialSlotId =
      Math.abs(face.normal.z) > 0.7 ? "roof" : "wall";
    if (!arrays.has(slot)) arrays.set(slot, newArrays());
    appendSolidFace(
      arrays.get(slot)!,
      face,
      resolveColor(slot, texturedSlots.has(slot)),
    );
  }
  for (const piece of parts.filter((part) => part.kind !== "mass")) {
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
      undefined,
      piece.rotation ?? 0,
    );
  }
  const geometries = new Map<MaterialSlotId, THREE.BufferGeometry>();
  for (const [slot, data] of arrays)
    geometries.set(slot, arraysToGeometry(data));
  return geometries;
}

export interface BuildPayloadOptions {
  parts: readonly BuildingPart[];
  /** 槽 → PNG 字节（用户绑定的游戏 raster 贴图）。 */
  textures?: ReadonlyMap<MaterialSlotId, Uint8Array<ArrayBuffer>>;
  buildingMaterial?: BuildingMaterialSource | null;
  /** Extra horizontal spacing between repeated window motifs, in metres. */
  windowSpacing?: number;
  /** 诊断文本里的来源说明（槽 → 描述）。 */
  textureLabels?: ReadonlyMap<MaterialSlotId, string>;
}

/** 部件 + 贴图绑定 → 合成 LotModelPayload（每槽一个 GLB mesh）。 */
export async function buildModelPayload(
  options: BuildPayloadOptions,
): Promise<LotModelPayload> {
  const { GLTFExporter } =
    await import("three/examples/jsm/exporters/GLTFExporter.js");
  if (options.buildingMaterial)
    return buildPropertyPayload(
      options.parts,
      options.buildingMaterial,
      options.windowSpacing ?? 0,
      new GLTFExporter(),
    );
  const texturedSlots = new Set(options.textures?.keys());
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
        (result) => resolve(result as ArrayBuffer),
        (error) =>
          reject(error instanceof Error ? error : new Error(String(error))),
        { binary: true },
      );
    });
    glbs.push(glb);
    const png = options.textures?.get(slot.id) ?? null;
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

/** Keep source binding indices; the ground slab is deliberately not a facade. */
async function buildPropertyPayload(
  parts: readonly BuildingPart[],
  source: BuildingMaterialSource,
  windowSpacing: number,
  exporter: import("three/examples/jsm/exporters/GLTFExporter.js").GLTFExporter,
): Promise<LotModelPayload> {
  const groups = new Map<number, MeshArrays>();
  const fallbackIndex = source.payload.materials.length;
  const arraysFor = (index: number) => {
    if (!groups.has(index)) groups.set(index, newArrays());
    return groups.get(index)!;
  };
  const emitFace = (face: SolidFace) => {
    for (const piece of spacedFacadeFaces(face, source, windowSpacing)) {
      appendSolidFace(
        arraysFor(piece.surface.materialIndex),
        piece.face,
        [1, 1, 1],
        piece.surface,
      );
    }
  };
  for (const face of buildMassFaces(parts)) emitFace(face);
  for (const part of parts.filter((part) => part.kind !== "mass")) {
    if (part.slot === "ground") {
      appendBox(
        arraysFor(fallbackIndex),
        part.center,
        part.size,
        defaultSlotColor("ground", false),
      );
    } else if (!(windowSpacing > 0)) {
      appendBox(
        newArrays(),
        part.center,
        part.size,
        [1, 1, 1],
        (normal) => {
          const surface =
            Math.abs(normal.z) > 0.7 ? source.roof : source.facade;
          return { target: arraysFor(surface.materialIndex), surface };
        },
        part.rotation ?? 0,
      );
    } else {
      for (const face of buildMassFaces([{ ...part, kind: "mass" }]))
        emitFace(face);
    }
  }
  const glbs: ArrayBuffer[] = [];
  const meshMaterialIndices: number[] = [];
  const meshUvKinds: number[] = [];
  for (const [materialIndex, arrays] of groups) {
    const geometry = arraysToGeometry(arrays);
    const material = new THREE.MeshStandardMaterial({ vertexColors: true });
    try {
      const mesh = new THREE.Mesh(geometry, material);
      mesh.name = `procedural-material-${materialIndex}`;
      glbs.push(
        await new Promise<ArrayBuffer>((resolve, reject) => {
          exporter.parse(
            mesh,
            (result) => resolve(result as ArrayBuffer),
            (error) => reject(error),
            { binary: true },
          );
        }),
      );
      meshMaterialIndices.push(materialIndex);
      meshUvKinds.push(materialIndex === fallbackIndex ? 1 : 2);
    } finally {
      geometry.dispose();
      material.dispose();
    }
  }
  const materials = [...source.payload.materials];
  if (groups.has(fallbackIndex))
    materials.push({
      baseColorPng: null,
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
  return {
    glbs,
    materials,
    meshMaterialIndices,
    meshUvKinds,
    diagnostics: `procedural building4: facade=${source.facade.materialIndex}:${source.facade.column}, roof=${source.roof.materialIndex}:${source.roof.column}; representative source UV/metre mapping`,
  };
}
