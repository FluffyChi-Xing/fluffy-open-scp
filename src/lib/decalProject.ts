import type * as ThreeNamespace from "three";
import type { DecalUnit } from "@/api/tauri";

type Three = typeof ThreeNamespace;

/**
 * 贴花在 lot 局部空间的投影坐标系。
 *
 * 12 float 变换是 WPF Matrix3D 行主序（行向量约定）：前 3 行是基向量、末行是平移。
 * 脱壳 exe 的 `decalMaterialInfoWithObjectData` 把贴花投影轴取为基的第三行，
 * 而 2026-09-14 用真实 lot（casino `0x457EA9DB`，模型 0x01532F56）实测：
 * 足迹内 3×3 射线**全部命中 +局部Z**且距离高度一致（离散度 0.00–2.05 m），
 * 即投影方向是 **+Z**，目标是一块平面。
 */
export interface DecalFrame {
  /** 贴花原点（lot 局部）。 */
  origin: ThreeNamespace.Vector3;
  /** 单位基向量（u / v / 投影轴）。 */
  axisX: ThreeNamespace.Vector3;
  axisY: ThreeNamespace.Vector3;
  axisZ: ThreeNamespace.Vector3;
  /** lot 局部变换矩阵（位置 + 朝向，无缩放）。 */
  matrix: ThreeNamespace.Matrix4;
  /** 投影盒 XY 全尺寸：`2×scale` 与 `2×scale/aspect`。 */
  sizeX: number;
  sizeY: number;
}

/** 盒的 Z 半厚下限/上限（米）。贴花只贴最近的一层表面，故取薄盒。 */
const HALF_THICKNESS_MIN = 0.5;
const HALF_THICKNESS_MAX = 2;
/** 足迹采样网格边长（3×3）。 */
const ANCHOR_GRID = 3;
/** 采样点占足迹的比例（留边，避免边界处打到相邻构件）。 */
const ANCHOR_SPAN = 0.6;
/** 射线最远距离（米）：超过即认为该方向没有目标面。 */
const ANCHOR_RAY_FAR = 60;

export function decalFrame(
  THREE: Three,
  unit: DecalUnit,
  aspectRatio: number,
): DecalFrame | null {
  const m = unit.transform?.matrix;
  if (!m || m.length !== 12) return null;
  const scale = unit.scale;
  if (scale === null || !Number.isFinite(scale) || scale <= 0) return null;
  const aspect = aspectRatio > 0 && Number.isFinite(aspectRatio) ? aspectRatio : 1;

  const axisX = new THREE.Vector3(m[0], m[1], m[2]);
  const axisY = new THREE.Vector3(m[3], m[4], m[5]);
  const axisZ = new THREE.Vector3(m[6], m[7], m[8]);
  const origin = new THREE.Vector3(m[9], m[10], m[11]);
  // 正交基（实测 |g| = 1）；退化时归一化保护
  axisX.normalize();
  axisY.normalize();
  axisZ.normalize();

  // 行主序 → three 列向量约定（同 unitGizmos.unitMatrix）
  const matrix = new THREE.Matrix4().set(
    m[0], m[3], m[6], m[9],
    m[1], m[4], m[7], m[10],
    m[2], m[5], m[8], m[11],
    0, 0, 0, 1,
  );

  const sizeX = Math.max(scale * 2, 0.05);
  return { origin, axisX, axisY, axisZ, matrix, sizeX, sizeY: Math.max(sizeX / aspect, 0.05) };
}

/**
 * 沿投影轴在足迹内做 3×3 射线，返回「原点到目标面」的锚定距离（命中中位数）。
 * 无命中返回 null。射线在 **lot 局部空间**进行（代理 Mesh 的 matrixWorld 恒等）。
 */
export function measureAnchorDistance(
  THREE: Three,
  frame: DecalFrame,
  proxies: ThreeNamespace.Mesh[],
): number | null {
  const raycaster = new THREE.Raycaster();
  raycaster.far = ANCHOR_RAY_FAR;
  const hits: number[] = [];
  const rayOrigin = new THREE.Vector3();
  for (let iy = 0; iy < ANCHOR_GRID; iy += 1) {
    for (let ix = 0; ix < ANCHOR_GRID; ix += 1) {
      const u = ((ix / (ANCHOR_GRID - 1)) * 2 - 1) * ANCHOR_SPAN * (frame.sizeX / 2);
      const v = ((iy / (ANCHOR_GRID - 1)) * 2 - 1) * ANCHOR_SPAN * (frame.sizeY / 2);
      rayOrigin
        .copy(frame.origin)
        .addScaledVector(frame.axisX, u)
        .addScaledVector(frame.axisY, v);
      for (const sign of [1, -1]) {
        raycaster.set(rayOrigin, frame.axisZ.clone().multiplyScalar(sign));
        const hit = raycaster.intersectObjects(proxies, false)[0];
        if (hit) {
          hits.push(hit.distance * sign);
          break;
        }
      }
    }
  }
  if (hits.length === 0) return null;
  hits.sort((a, b) => a - b);
  return hits[Math.floor(hits.length / 2)];
}

/**
 * 生成投影盒（lot 局部）：中心锚在目标面上，Z 取薄盒，避免穿透到背面。
 * 不剔背面是照抄引擎 `decalProject` 的 `clip(1-abs(tex))`（无法线判定），
 * 因此盒厚度就是唯一的防穿透手段。
 */
export function decalProjector(
  THREE: Three,
  frame: DecalFrame,
  anchor: number,
  depth: number | null,
): {
  position: ThreeNamespace.Vector3;
  orientation: ThreeNamespace.Euler;
  size: ThreeNamespace.Vector3;
} {
  const thickness = Math.min(
    Math.max(depth ?? 0, HALF_THICKNESS_MIN),
    HALF_THICKNESS_MAX,
  );
  const position = frame.origin.clone().addScaledVector(frame.axisZ, anchor);
  const orientation = new THREE.Euler().setFromRotationMatrix(frame.matrix);
  const size = new THREE.Vector3(frame.sizeX, frame.sizeY, thickness * 2);
  return { position, orientation, size };
}

/**
 * 把贴花投影到建筑几何上，返回 lot 局部空间的合并几何；无命中返回 null。
 *
 * `meshes` 是建筑网格；内部用**恒等代理 Mesh** 喂给 `DecalGeometry`——
 * 它在 `pushDecalVertex` 里做 `vertex.applyMatrix4(mesh.matrixWorld)`、最后再用
 * 投影矩阵乘回，直接传真实 mesh 会把结果抛到 renderer 世界空间（`viewer.world`
 * 带 -90°X）。代理的 matrixWorld 恒等 ⇒ 顶点空间 = lot 局部 ⇒ 输出即 lot 局部。
 */
export async function projectDecal(
  THREE: Three,
  frame: DecalFrame,
  meshes: ThreeNamespace.Mesh[],
  depth: number | null,
): Promise<ThreeNamespace.BufferGeometry | null> {
  if (meshes.length === 0) return null;
  const proxies = meshes.map((mesh) => new THREE.Mesh(mesh.geometry));
  const anchor = measureAnchorDistance(THREE, frame, proxies);
  if (anchor === null) return null;

  const { position, orientation, size } = decalProjector(THREE, frame, anchor, depth);
  const { DecalGeometry } = await import(
    "three/examples/jsm/geometries/DecalGeometry.js"
  );
  const boxAabb = boxBounds(THREE, position, orientation, size);
  const pieces: ThreeNamespace.BufferGeometry[] = [];
  for (const proxy of proxies) {
    const geometry = proxy.geometry as ThreeNamespace.BufferGeometry;
    geometry.computeBoundingBox();
    const bb = geometry.boundingBox;
    if (bb && !aabbOverlaps(boxAabb, bb)) continue;
    const piece = new DecalGeometry(proxy, position, orientation, size);
    if (piece.attributes.position && piece.attributes.position.count > 0) pieces.push(piece);
  }
  if (pieces.length === 0) return null;

  const merged = await mergePieces(pieces);
  // 引擎 UV 是 `texturePosition.xy * -0.5 + 0.5`（两轴取负）；DecalGeometry 输出
  // `0.5 + x/size.x`，与 PlaneGeometry 逐轴同向 ⇒ 沿用已验证的「只镜像 U」
  //（v 由 TextureLoader 的 flipY=true 抵消）。
  const uv = merged.attributes.uv;
  if (uv) {
    for (let i = 0; i < uv.count; i += 1) uv.setX(i, 1 - uv.getX(i));
    uv.needsUpdate = true;
  }
  for (const piece of pieces) {
    if (piece !== merged) piece.dispose();
  }
  return merged;
}

async function mergePieces(
  pieces: ThreeNamespace.BufferGeometry[],
): Promise<ThreeNamespace.BufferGeometry> {
  if (pieces.length === 1) return pieces[0];
  const { mergeGeometries } = await import(
    "three/examples/jsm/utils/BufferGeometryUtils.js"
  );
  const merged = mergeGeometries(pieces, false);
  if (merged) return merged;
  // 属性不一致时退化为第一片（几何仍可用）
  return pieces[0];
}

function boxBounds(
  THREE: Three,
  position: ThreeNamespace.Vector3,
  orientation: ThreeNamespace.Euler,
  size: ThreeNamespace.Vector3,
): ThreeNamespace.Box3 {
  const matrix = new THREE.Matrix4().makeRotationFromEuler(orientation);
  matrix.setPosition(position);
  const box = new THREE.Box3();
  const corner = new THREE.Vector3();
  for (const sx of [-0.5, 0.5]) {
    for (const sy of [-0.5, 0.5]) {
      for (const sz of [-0.5, 0.5]) {
        corner.set(sx * size.x, sy * size.y, sz * size.z).applyMatrix4(matrix);
        box.expandByPoint(corner);
      }
    }
  }
  return box;
}

function aabbOverlaps(
  a: ThreeNamespace.Box3,
  b: ThreeNamespace.Box3,
): boolean {
  return !(
    a.max.x < b.min.x ||
    a.min.x > b.max.x ||
    a.max.y < b.min.y ||
    a.min.y > b.max.y ||
    a.max.z < b.min.z ||
    a.min.z > b.max.z
  );
}
