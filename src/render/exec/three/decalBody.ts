/**
 * 贴花装配执行器（three）：体积盒投影 / 浮空 quad / 破洞灯光。
 *
 * 材质路由与几何来自 `plan/decal.ts`（family/surface/frame）；
 * 本层承接旧视口 `buildDecalObject` 的 three 细节（DecalGeometry、
 * 法线过滤、引擎材质、fallback 材质、hole SpotLight）。
 */
import type * as ThreeNamespace from "three";
import type { DecalUnit, DecalUnitTexture } from "@/api/tauri";
import type { SunEnvRefs } from "@/pages/packages/components/property-editor/refinedRender";
import { decalHalfThickness } from "@/lib/decalProject";
import { createEngineDecalMaterial } from "@/lib/decalEngineMaterials";
import type { EngineFamily } from "@/lib/decalEngineMaterials";
import { decalFramePlan, decalRoutePlan } from "../../plan/decal";
import { decalFrameObject } from "./decal";
import { rowMajor12ToTrs } from "../../resolve/transform";

export const HOLE_LIGHT_MAX = 8;
const DECAL_MATERIAL_VARIANTS: Record<number, "sign" | "graffiti"> = {
  0x73684efc: "sign",
  0xe5390a98: "graffiti",
};
/** 只保留 N·axisZ ≤ 0 的面（见旧注释：远侧面/切向面裁剪）。 */
const DECAL_PROJECTION_NORMAL_CUTOFF = 0;

export interface DecalBodyState {
  /** 本轮投影数（遥测） */
  projected: number;
  /** 浮空 quad 数（遥测） */
  fallback: number;
  /** 已消费的 hole 灯数量（上限 HOLE_LIGHT_MAX） */
  holeLightCount: number;
  /** SDF 族需要 rAF 时钟 */
  neonAnimated: boolean;
}

export interface DecalBodyInput {
  readonly THREE: typeof ThreeNamespace;
  readonly unit: DecalUnit;
  readonly texture: DecalUnitTexture;
  readonly decoded: ThreeNamespace.Texture;
  readonly proxies: ThreeNamespace.Mesh[];
  readonly env: SunEnvRefs;
  readonly engineMaterials: boolean;
  readonly decalLight: [number, number] | null;
  readonly powered: boolean;
  readonly isStale: () => boolean;
  readonly state: DecalBodyState;
}

/** 按面法线过滤投影几何（只留朝原点的面）。 */
export function filterDecalProjection(
  THREE: typeof ThreeNamespace,
  geometry: ThreeNamespace.BufferGeometry,
  axisZ: ThreeNamespace.Vector3,
): ThreeNamespace.BufferGeometry | null {
  const pos = geometry.attributes.position;
  const nrm = geometry.attributes.normal;
  const uv = geometry.attributes.uv;
  if (!pos || !nrm || !uv) return geometry;
  const positions: number[] = [];
  const normals: number[] = [];
  const uvs: number[] = [];
  for (let i = 0; i < pos.count; i += 3) {
    const dot =
      ((nrm.getX(i) + nrm.getX(i + 1) + nrm.getX(i + 2)) / 3) * axisZ.x +
      ((nrm.getY(i) + nrm.getY(i + 1) + nrm.getY(i + 2)) / 3) * axisZ.y +
      ((nrm.getZ(i) + nrm.getZ(i + 1) + nrm.getZ(i + 2)) / 3) * axisZ.z;
    if (dot > -DECAL_PROJECTION_NORMAL_CUTOFF) continue;
    for (let k = 0; k < 3; k += 1) {
      positions.push(pos.getX(i + k), pos.getY(i + k), pos.getZ(i + k));
      normals.push(nrm.getX(i + k), nrm.getY(i + k), nrm.getZ(i + k));
      uvs.push(uv.getX(i + k), uv.getY(i + k));
    }
  }
  if (!positions.length) return null;
  const filtered = new THREE.BufferGeometry();
  filtered.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  filtered.setAttribute("normal", new THREE.Float32BufferAttribute(normals, 3));
  filtered.setAttribute("uv", new THREE.Float32BufferAttribute(uvs, 2));
  return filtered;
}

/** 引擎材质不可用时的回退材质（sign/graffiti/默认）。 */
export function buildDecalFallbackMaterial(
  THREE: typeof ThreeNamespace,
  dto: DecalUnitTexture,
  map: ThreeNamespace.Texture,
  opts: {
    halfDepth?: number;
    boxHalfX?: number;
    boxHalfY?: number;
    decalData: [number, number] | null;
    env: SunEnvRefs;
    depthTest?: boolean;
    depthWrite?: boolean;
    alphaTest?: number;
  },
): ThreeNamespace.Material {
  const base = {
    map,
    side: THREE.DoubleSide,
    transparent: true,
    depthTest: opts.depthTest ?? true,
    depthWrite: opts.depthWrite ?? false,
    alphaTest: opts.alphaTest ?? 0,
    polygonOffset: true,
    polygonOffsetFactor: -4,
    polygonOffsetUnits: -4,
  };
  switch (DECAL_MATERIAL_VARIANTS[(dto.materialInstance ?? 0) >>> 0]) {
    case "sign":
      return new THREE.MeshLambertMaterial({ ...base, color: new THREE.Color(2, 2, 2) });
    case "graffiti":
      return new THREE.MeshStandardMaterial({ ...base, roughness: 1, metalness: 0 });
    default:
      return new THREE.MeshStandardMaterial({ ...base, roughness: 1, metalness: 0 });
  }
}

export async function buildDecalBody(
  input: DecalBodyInput,
): Promise<ThreeNamespace.Object3D | null> {
  const { THREE, unit, texture, decoded, proxies, env, state } = input;
  if (!texture.png) {
    console.warn("[decal] DTO 无 png：", texture.idInstance, texture.error ?? "unknown");
    return null;
  }
  const aspect = texture.aspectRatio && texture.aspectRatio > 0 ? texture.aspectRatio : 1;
  const framePlan = decalFramePlan(unit.transform?.matrix, unit.scale, unit.depth, aspect);
  const frame = framePlan ? decalFrameObject(THREE, framePlan) : null;
  if (!frame) {
    console.warn(
      "[decal] 无有效 transform/scale，无法构建投影帧：",
      `decal:${unit.category}:${unit.index}`,
      "transform=",
      unit.transform ? `matrixLen=${unit.transform.matrix?.length ?? 0}` : "null",
      "scale=",
      unit.scale,
    );
    return null;
  }

  const group = new THREE.Group();
  const trs = unit.transform ? rowMajor12ToTrs(unit.transform.matrix) : null;
  if (trs) {
    group.position.set(trs.position[0], trs.position[1], trs.position[2]);
    group.quaternion.set(
      trs.quaternion[0],
      trs.quaternion[1],
      trs.quaternion[2],
      trs.quaternion[3],
    );
    group.scale.set(trs.scale[0], trs.scale[1], trs.scale[2]);
  }

  const halfScale = frame.sizeY / 2;
  const dataDepth =
    unit.depth !== null && Number.isFinite(unit.depth) && unit.depth > 0
      ? unit.depth * halfScale
      : null;
  const { DecalGeometry } = await import(
    "three/examples/jsm/geometries/DecalGeometry.js"
  );
  if (input.isStale()) return null;
  const route = decalRoutePlan(texture.materialInstance);
  const family: EngineFamily = route.family;
  if (family === "sdf") state.neonAnimated = true;

  const addFloatQuad = () => {
    const floatMaterial = input.engineMaterials
      ? createEngineDecalMaterial(THREE, family, {
          map: decoded,
          layerColors: texture.colors,
          decalData: input.decalLight,
          worldDirection: frame.axisZ,
          env,
          materialInfo: unit.materialData ?? undefined,
          powered: input.powered,
          nus: [frame.sizeX, frame.sizeY, frame.sizeY],
          graffiti: (texture.materialInstance ?? 0) >>> 0 === 0xe5390a98,
          depthTest: true,
          depthWrite: true,
          alphaTest: 0.01,
        })
      : buildDecalFallbackMaterial(THREE, texture, decoded, {
          env,
          halfDepth: decalHalfThickness(unit.depth),
          boxHalfX: frame.sizeX / 2,
          boxHalfY: frame.sizeY / 2,
          decalData: input.decalLight,
          depthTest: true,
          depthWrite: true,
          alphaTest: 0.01,
        });
    const quadGeometry = new THREE.PlaneGeometry(frame.sizeX, frame.sizeY);
    const quadUv = quadGeometry.attributes.uv;
    for (let i = 0; i < quadUv.count; i += 1) quadUv.setX(i, 1 - quadUv.getX(i));
    quadUv.needsUpdate = true;
    const quad = new THREE.Mesh(quadGeometry, floatMaterial);
    quad.position.z = (dataDepth ?? 0) / 2;
    group.add(quad);
    state.fallback += 1;
  };
  if (route.surface === "plane") addFloatQuad();

  const depthZ = dataDepth ?? halfScale * 2.5;
  const projector = {
    position: frame.origin.clone().addScaledVector(frame.axisZ, depthZ / 2),
    orientation: new THREE.Euler().setFromRotationMatrix(frame.matrix),
    size: new THREE.Vector3(frame.sizeX, frame.sizeY, depthZ),
  };
  const boxRadius = Math.hypot(projector.size.x, projector.size.y, projector.size.z) / 2;
  const geometries: ThreeNamespace.BufferGeometry[] = [];
  for (const proxy of proxies) {
    const position = proxy.geometry.getAttribute("position");
    if (!position || !Number.isFinite(position.count) || position.count === 0) continue;
    let validPosition = true;
    for (let i = 0; i < position.count && validPosition; i += 1) {
      validPosition =
        Number.isFinite(position.getX(i)) &&
        Number.isFinite(position.getY(i)) &&
        Number.isFinite(position.getZ(i));
    }
    if (!validPosition) continue;
    if (!proxy.geometry.boundingSphere) proxy.geometry.computeBoundingSphere();
    const sphere = proxy.geometry.boundingSphere;
    if (
      sphere &&
      sphere.center.distanceTo(projector.position) > sphere.radius + boxRadius
    ) {
      continue;
    }
    const clipped = new DecalGeometry(
      proxy,
      projector.position,
      projector.orientation,
      projector.size,
    );
    const clippedPosition = clipped.getAttribute("position");
    if (clippedPosition) {
      let finite = true;
      for (let i = 0; i < clippedPosition.count && finite; i += 1) {
        finite =
          Number.isFinite(clippedPosition.getX(i)) &&
          Number.isFinite(clippedPosition.getY(i)) &&
          Number.isFinite(clippedPosition.getZ(i));
      }
      if (!finite) {
        clipped.dispose();
        continue;
      }
    }
    if ((clipped.attributes.position?.count ?? 0) === 0) continue;
    const filtered =
      family === "hole" || route.surface === "plane"
        ? clipped
        : filterDecalProjection(THREE, clipped, frame.axisZ);
    if (filtered !== clipped) clipped.dispose();
    if (filtered) geometries.push(filtered);
  }
  if (!geometries.length) return group;
  for (const geometry of geometries) {
    const uv = geometry.attributes.uv;
    for (let i = 0; i < uv.count; i += 1) uv.setX(i, 1 - uv.getX(i));
    uv.needsUpdate = true;
  }
  const engineMaterial = input.engineMaterials
    ? createEngineDecalMaterial(
        THREE,
        route.surface === "plane" ? "neon-light" : family,
        {
          map: decoded,
          layerColors: texture.colors,
          decalData: input.decalLight,
          worldDirection: frame.axisZ,
          env,
          materialInfo: unit.materialData ?? undefined,
          powered: input.powered,
          nus: [2 / frame.sizeX, 2 / frame.sizeY, 2 / depthZ],
          graffiti: (texture.materialInstance ?? 0) >>> 0 === 0xe5390a98,
          side: THREE.FrontSide,
          holeBox:
            family === "hole" || route.surface === "plane"
              ? {
                  origin: frame.origin,
                  axisZ: frame.axisZ,
                  invDepth: 1 / Math.max(depthZ, 0.01),
                }
              : undefined,
        },
      )
    : buildDecalFallbackMaterial(THREE, texture, decoded, {
        env,
        halfDepth: decalHalfThickness(unit.depth),
        boxHalfX: frame.sizeX / 2,
        boxHalfY: frame.sizeY / 2,
        decalData: input.decalLight,
      });
  const container = new THREE.Group();
  container.matrix.copy(frame.matrix).invert();
  container.matrixAutoUpdate = false;
  for (const geometry of geometries) {
    container.add(new THREE.Mesh(geometry, engineMaterial));
  }
  group.add(container);
  state.projected += 1;
  if (family === "hole" && input.decalLight && state.holeLightCount < HOLE_LIGHT_MAX) {
    const [scaleFactor, radiusFactor] = input.decalLight;
    const spot = new THREE.SpotLight(
      0xffdca0,
      (scaleFactor * 16 + 1) * 3,
      radiusFactor * 8,
      0.9,
      0.6,
      1,
    );
    spot.map = decoded;
    spot.position.set(0, 0, -0.5);
    spot.target.position.set(0, 0, 1);
    group.add(spot, spot.target);
    state.holeLightCount += 1;
  }
  return group;
}
