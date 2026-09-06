import type * as ThreeNamespace from "three";
import type {
  DecalUnit,
  LightUnit,
  LotUnitDto,
  PathPointUnit,
  UnitTransformDto,
} from "@/api/tauri";

type Three = typeof ThreeNamespace;

export const PATH_LINE_COLOR = 0x00c8c8;
export const EFFECT_COLOR = 0xffd700;
export const PROP_COLOR = 0xc81e1e;
export const SPAWNER_COLOR = 0x1e40c8;
export const DECAL_BACK_COLOR = 0x33cc33;
export const SELECTED_EMISSIVE = 0x2f5fb0;

/**
 * Unit 稳定标识：视口 userData、Outliner、选中态共用。
 * prop/decal 有分箱/类别维度，id 携带 bin/category。
 */
export function unitId(unit: LotUnitDto): string {
  if (unit.kind === "prop") return `prop:${unit.bin}:${unit.index}`;
  if (unit.kind === "decal") return `decal:${unit.category}:${unit.index}`;
  return `${unit.kind}:${unit.index}`;
}

/**
 * WPF Matrix3D（行主序、行向量约定、平移在末行）→ three Matrix4。
 * 列向量约定需转置：3×3 转置、平移入第 4 列。
 */
export function unitMatrix(
  THREE: Three,
  transform: UnitTransformDto,
): ThreeNamespace.Matrix4 {
  const m = transform.matrix;
  if (m.length !== 12) return new THREE.Matrix4();
  return new THREE.Matrix4().set(
    m[0], m[3], m[6], m[9],
    m[1], m[4], m[7], m[10],
    m[2], m[5], m[8], m[11],
    0, 0, 0, 1,
  );
}

function applyTransform(
  THREE: Three,
  object: ThreeNamespace.Object3D,
  transform: UnitTransformDto | null,
) {
  if (!transform) return;
  const matrix = unitMatrix(THREE, transform);
  matrix.decompose(object.position, object.quaternion, object.scale);
}

function standardMaterial(
  THREE: Three,
  color: number,
  opacity = 1,
): ThreeNamespace.MeshStandardMaterial {
  return new THREE.MeshStandardMaterial({
    color,
    roughness: 0.6,
    metalness: 0.05,
    transparent: opacity < 1,
    opacity,
    side: THREE.DoubleSide,
  });
}

/**
 * SCP 单元图元显示约定（真实包 + 用户对拍逐轮校准，见 migration.md §15：
 * 数据帧 Z-up、行向量 p·M，SCP CreateGeometry 中的 R(±90) 为无效死代码）：
 * Point=球 r1；Spot=截锥开口沿 M 第 2 行（局部 +Y，无预旋转）；
 * Line=盒长轴沿 M 第 2 行（同为局部 +Y，盒中心偏移 (0,0,-len/2) 随行向量生效）。
 * 保存侧的反向补偿属 M-PE2，写回时另做。
 */
function buildLight(THREE: Three, unit: LightUnit): ThreeNamespace.Object3D {
  const color = unit.color ?? [1, 1, 1];
  const rgb = new THREE.Color(color[0], color[1], color[2]);
  let geometry: ThreeNamespace.BufferGeometry;
  let opacity = 1;
  if (unit.lightType === "Spot") {
    const length = Math.max(unit.length ?? 0, 0.05);
    const radius = Math.max(unit.outerRadius ?? 1, 0.05);
    geometry = new THREE.CylinderGeometry(radius, 0.001, length, 24, 1, false);
    geometry.translate(0, length / 2, 0);
    opacity = 0.6;
  } else if (unit.lightType === "Line") {
    const length = Math.max(unit.length ?? 0, 0.05);
    geometry = new THREE.BoxGeometry(1, length, 1);
    // 灯带从灯具原点沿发光方向（局部 +Y → M 第 2 行）延伸整段长度；
    // 上一版沿用的 Helix Center(0,0,-len/2) 偏移会经 M 映射出半个长度的漂移。
    geometry.translate(0, length / 2, 0);
  } else {
    geometry = new THREE.SphereGeometry(1, 24, 16);
  }
  const mesh = new THREE.Mesh(geometry, standardMaterial(THREE, rgb.getHex(), opacity));
  applyTransform(THREE, mesh, unit.transform);
  return mesh;
}

/**
 * Effect/Prop/Spawner 共用的标记锥（h2.5、开口半径 1）。
 * 预旋转 +90°X 使宽端沿 +M 第 3 行：恒等变换下宽端朝上、尖锥向下（▼，
 * 对齐原 SCP 截图；上一轮 -90°X 曾导致上下颠倒）。
 */
function buildMarkerCone(
  THREE: Three,
  color: number,
  transform: UnitTransformDto | null,
): ThreeNamespace.Mesh {
  const geometry = new THREE.CylinderGeometry(1, 0.001, 2.5, 24, 1, false);
  geometry.translate(0, 1.25, 0);
  geometry.rotateX(Math.PI / 2);
  const mesh = new THREE.Mesh(geometry, standardMaterial(THREE, color));
  applyTransform(THREE, mesh, transform);
  return mesh;
}

/** 贴花矩形：正面近透明、背面绿色（同 SCP RectangleVisual3D），尺寸 2×Scale。 */
function buildDecal(THREE: Three, unit: DecalUnit): ThreeNamespace.Group {
  const size = Math.max((unit.scale ?? 1) * 2, 0.05);
  const group = new THREE.Group();
  const front = new THREE.Mesh(
    new THREE.PlaneGeometry(size, size),
    standardMaterial(THREE, 0xffffff, 0.06),
  );
  const backGeometry = new THREE.PlaneGeometry(size, size);
  backGeometry.rotateY(Math.PI);
  const back = new THREE.Mesh(
    backGeometry,
    standardMaterial(THREE, DECAL_BACK_COLOR, 0.4),
  );
  group.add(front, back);
  applyTransform(THREE, group, unit.transform);
  return group;
}

function buildPathPoint(
  THREE: Three,
  unit: PathPointUnit,
): ThreeNamespace.Mesh {
  const geometry = new THREE.SphereGeometry(2, 20, 14);
  const mesh = new THREE.Mesh(geometry, standardMaterial(THREE, PATH_LINE_COLOR));
  if (unit.point) {
    mesh.position.set(unit.point[0], unit.point[1], unit.point[2]);
  }
  return mesh;
}

/** 路径折线：按 point_index 排序连接各点（`pathPairs` 语义未定，先作 best-effort）。 */
export function buildPathLine(
  THREE: Three,
  points: ThreeNamespace.Vector3[],
): ThreeNamespace.Line | null {
  if (points.length < 2) return null;
  const geometry = new THREE.BufferGeometry().setFromPoints(points);
  return new THREE.Line(
    geometry,
    new THREE.LineBasicMaterial({ color: PATH_LINE_COLOR }),
  );
}

/**
 * 精细渲染：真实 three.js 光源（Point/Spot/Line）。
 * Spot 锥轴 = 局部 +Y（与标记锥同约定）；强度/衰减为观感近似值，待用户对拍校准。
 * Point/Spot 保留小发光球作灯具示意；Line 为氛围光——**本体完全不可见**，
 * 沿灯带均匀布点近似"照亮周围"，仅留透明拾取代理供选择。
 */
export function buildRealLightUnit(
  THREE: Three,
  unit: LightUnit,
): ThreeNamespace.Object3D {
  const group = new THREE.Group();
  const color = unit.color ?? [1, 1, 1];
  const rgb = new THREE.Color(color[0], color[1], color[2]);
  const radius = Math.max(unit.outerRadius ?? 4, 1);
  const length = Math.max(unit.length ?? radius, 0.5);
  const intensity = Math.max(unit.diffuse ?? 1, 0.05) * 8;

  if (unit.lightType === "Spot") {
    const angle = Math.min(Math.atan2(radius, length), 1.45);
    const spot = new THREE.SpotLight(rgb.getHex(), intensity, radius * 2, angle, 0.5, 1);
    spot.target.position.set(0, length, 0);
    group.add(spot, spot.target);
  } else if (unit.lightType === "Line") {
    // 沿灯带（局部 +Y 自原点延伸）均匀布 N 个小范围点光，近似条形氛围照明
    const segments = Math.min(Math.max(Math.ceil(length / 8), 2), 6);
    for (let index = 0; index < segments; index += 1) {
      const light = new THREE.PointLight(
        rgb.getHex(),
        intensity / segments,
        Math.max(length, radius) * 1.2,
        1,
      );
      light.position.set(0, (length * (index + 0.5)) / segments, 0);
      group.add(light);
    }
  } else {
    group.add(new THREE.PointLight(rgb.getHex(), intensity, radius * 2, 1));
  }

  if (unit.lightType === "Line") {
    // 不可见拾取代理（透明不写深度；Raycaster 不过滤透明对象）
    const proxyGeometry = new THREE.BoxGeometry(2, length, 2);
    proxyGeometry.translate(0, length / 2, 0);
    group.add(
      new THREE.Mesh(
        proxyGeometry,
        new THREE.MeshBasicMaterial({ transparent: true, opacity: 0, depthWrite: false }),
      ),
    );
  } else {
    group.add(
      new THREE.Mesh(
        new THREE.SphereGeometry(1, 16, 12),
        new THREE.MeshBasicMaterial({ color: rgb }),
      ),
    );
  }
  applyTransform(THREE, group, unit.transform);
  return group;
}

/** 由 Unit DTO 构建视口对象；userData 记录 unitId/kind 供拾取与可见性控制。 */
export function buildUnitObject(
  THREE: Three,
  unit: LotUnitDto,
): ThreeNamespace.Object3D | null {
  let object: ThreeNamespace.Object3D | null = null;
  switch (unit.kind) {
    case "light":
      object = buildLight(THREE, unit);
      break;
    case "effect":
      object = buildMarkerCone(THREE, EFFECT_COLOR, unit.transform);
      break;
    case "prop":
      object = buildMarkerCone(THREE, PROP_COLOR, unit.transform);
      break;
    case "spawner":
      object = buildMarkerCone(THREE, SPAWNER_COLOR, unit.transform);
      break;
    case "decal":
      object = buildDecal(THREE, unit);
      break;
    case "pathPoint":
      object = buildPathPoint(THREE, unit);
      break;
  }
  if (object) {
    object.userData.unitId = unitId(unit);
    object.userData.unitKind = unit.kind;
  }
  return object;
}
