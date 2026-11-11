/**
 * 编辑器覆盖层执行器（three）：Unit 标记锥 / 灯位代理 / 贴花矩形 / 路径线 /
 * 真实光源。迁移自旧 `unitGizmos.ts` 的构建器，行为逐项对齐。
 *
 * 说明：`unitId` / `unitMatrix` 仍留在旧模块（会话/属性面板共用）；本模块
 * 用 `plan/units` 的稳定 id 与纯 TRS，不再依赖旧矩阵工具。
 */
import type * as ThreeNamespace from "three";
import type {
  DecalUnit,
  LightUnit,
  LotUnitDto,
  PathPointUnit,
  UnitTransformDto,
} from "@/api/tauri";
import { unitPlanId } from "../../plan/units";
import { rowMajor12ToTrs } from "../../resolve/transform";
import { createLightVolumeMesh, lightVolumeIntensity } from "./lightVolume";

type Three = typeof ThreeNamespace;

export const PATH_LINE_COLOR = 0x00c8c8;
export const EFFECT_COLOR = 0xffd700;
export const PROP_COLOR = 0xc81e1e;
export const SPAWNER_COLOR = 0x1e40c8;
export const DECAL_BACK_COLOR = 0x33cc33;

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function spawnerSeed(unit: LotUnitDto): number {
  const id = unitPlanId(unit);
  let hash = 0x811c9dc5;
  for (let i = 0; i < id.length; i += 1) {
    hash ^= id.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

/** WPF 行主序 12 floats → TRS（与旧 unitMatrix().decompose 等价）。 */
function applyTransform(
  THREE: Three,
  object: ThreeNamespace.Object3D,
  transform: UnitTransformDto | null | undefined,
): void {
  if (!transform) return;
  const trs = rowMajor12ToTrs(transform.matrix);
  if (!trs) return;
  object.position.set(trs.position[0], trs.position[1], trs.position[2]);
  object.quaternion.set(
    trs.quaternion[0],
    trs.quaternion[1],
    trs.quaternion[2],
    trs.quaternion[3],
  );
  object.scale.set(trs.scale[0], trs.scale[1], trs.scale[2]);
}

function standardMaterial(THREE: Three, color: number, opacity = 1): ThreeNamespace.MeshStandardMaterial {
  return new THREE.MeshStandardMaterial({
    color,
    roughness: 0.6,
    metalness: 0.05,
    transparent: opacity < 1,
    opacity,
    side: THREE.DoubleSide,
  });
}

const SIM_SKIN_TONES = [0xe8b89a, 0xd9a077, 0xb97a50, 0x8a5a38, 0x6b4226];
const SIM_OUTFIT_COLORS = [
  0xc81e1e, 0x1e40c8, 0x20c050, 0xf0a000, 0x8848c8, 0x20b0b0, 0xe8e8e8,
  0x303038, 0xc84890,
];
const SIM_PANTS_COLORS = [0x303040, 0x40485c, 0x242428, 0x54423a];

function buildSimFigure(THREE: Three, seed: number): ThreeNamespace.Object3D {
  const rand = mulberry32(seed);
  const skin = SIM_SKIN_TONES[Math.floor(rand() * SIM_SKIN_TONES.length)];
  const outfit = SIM_OUTFIT_COLORS[Math.floor(rand() * SIM_OUTFIT_COLORS.length)];
  const pants = SIM_PANTS_COLORS[Math.floor(rand() * SIM_PANTS_COLORS.length)];
  const jitter = 0.92 + rand() * 0.16;
  const group = new THREE.Group();
  const addLimb = (
    geometry: ThreeNamespace.CylinderGeometry | ThreeNamespace.SphereGeometry,
    color: number,
    y: number,
    x = 0,
  ) => {
    const mesh = new THREE.Mesh(geometry, standardMaterial(THREE, color));
    mesh.position.set(x, y, 0);
    group.add(mesh);
  };
  addLimb(new THREE.CylinderGeometry(0.085, 0.1, 0.78, 10), pants, 0.39, -0.11);
  addLimb(new THREE.CylinderGeometry(0.085, 0.1, 0.78, 10), pants, 0.39, 0.11);
  addLimb(new THREE.CylinderGeometry(0.17, 0.21, 0.62, 12), outfit, 1.09);
  addLimb(new THREE.CylinderGeometry(0.055, 0.065, 0.58, 8), outfit, 1.06, -0.27);
  addLimb(new THREE.CylinderGeometry(0.055, 0.065, 0.58, 8), outfit, 1.06, 0.27);
  addLimb(new THREE.SphereGeometry(0.155, 14, 12), skin, 1.62);
  group.scale.setScalar(jitter);
  return group;
}

export function buildSpawnerPlaceholder(
  THREE: Three,
  unit: LotUnitDto,
): ThreeNamespace.Object3D {
  const figure = buildSimFigure(THREE, spawnerSeed(unit));
  if (unit.kind !== "pathPoint") applyTransform(THREE, figure, unit.transform);
  return figure;
}

/** 灯位几何兜底：length 缺失时退回外半径（此前 0.05 m 的锥体 = 看不见，
 *  放置灯在 default 模式"偶尔渲染不出来"的直接原因）。 */
function lightLength(unit: LightUnit): number {
  return Math.max(unit.length ?? unit.outerRadius ?? 4, 0.5);
}

function lightRadius(unit: LightUnit): number {
  return Math.max(unit.outerRadius ?? lightLength(unit) * 0.5, 0.1);
}

/** 灯位本体（小而亮的球），保证在夜景/白模下都能看见并可拾取。 */
function buildLightBulb(
  THREE: Three,
  rgb: ThreeNamespace.Color,
  radius: number,
): ThreeNamespace.Mesh {
  const bulb = new THREE.Mesh(
    new THREE.SphereGeometry(Math.max(radius * 0.07, 0.22), 16, 12),
    new THREE.MeshBasicMaterial({ color: rgb, toneMapped: false }),
  );
  bulb.name = "__lightBulb";
  return bulb;
}

function buildLight(THREE: Three, unit: LightUnit): ThreeNamespace.Object3D {
  const color = unit.color ?? [1, 1, 1];
  const rgb = new THREE.Color(color[0], color[1], color[2]);
  const group = new THREE.Group();
  if (unit.lightType === "Spot") {
    const length = lightLength(unit);
    const radius = lightRadius(unit);
    // Spot beams are volumetric by default (game light volumes are cones);
    // an explicit isVolumetric=false keeps a thin marker instead.
    if (unit.isVolumetric !== false) {
      const beam = createLightVolumeMesh(THREE, {
        color: rgb,
        length,
        radius,
        intensity: lightVolumeIntensity(unit.diffuse, unit.volumetricStrength),
      });
      group.add(beam.mesh);
    } else {
      const geometry = new THREE.CylinderGeometry(radius, 0.001, length, 24, 1, true);
      geometry.translate(0, length / 2, 0);
      const marker = new THREE.Mesh(
        geometry,
        new THREE.MeshBasicMaterial({
          color: rgb,
          transparent: true,
          opacity: 0.16,
          depthWrite: false,
          side: THREE.DoubleSide,
          toneMapped: false,
        }),
      );
      marker.raycast = () => {};
      marker.userData.pickOnly = true;
      group.add(marker);
    }
    group.add(buildLightBulb(THREE, rgb, radius));
  } else if (unit.lightType === "Line") {
    const length = lightLength(unit);
    const geometry = new THREE.BoxGeometry(1, length, 1);
    geometry.translate(0, length / 2, 0);
    group.add(new THREE.Mesh(geometry, standardMaterial(THREE, rgb.getHex())));
  } else {
    group.add(buildLightBulb(THREE, rgb, lightRadius(unit)));
  }
  applyTransform(THREE, group, unit.transform);
  return group;
}

function buildMarkerCone(
  THREE: Three,
  color: number,
  transform: UnitTransformDto | null,
): ThreeNamespace.Object3D {
  const geometry = new THREE.CylinderGeometry(1, 0.001, 2.5, 24, 1, false);
  geometry.translate(0, 1.25, 0);
  geometry.rotateX(Math.PI / 2);
  const mesh = new THREE.Mesh(geometry, standardMaterial(THREE, color));
  applyTransform(THREE, mesh, transform);
  return mesh;
}

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

/** 画质档裁切距离（米，用户实测：低→高 = Near/Mid/Far/Max）。 */
export const LIGHT_CULL_DISTANCE: Record<"Near" | "Mid" | "Far" | "Max", number> = {
  Near: 10,
  Mid: 25,
  Far: 50,
  Max: 100,
};

export function buildRealLightUnit(
  THREE: Three,
  unit: LightUnit,
): ThreeNamespace.Object3D {
  const group = new THREE.Group();
  const color = unit.color ?? [1, 1, 1];
  const rgb = new THREE.Color(color[0], color[1], color[2]);
  const radius = Math.max(unit.outerRadius ?? 4, 1);
  const length = lightLength(unit);
  // 游戏侧每盏灯的打包契约（`docs/re/light-uniform-contract-20261111.md`，
  // 从 App.package 的渲染器 HLSL 反编译得到）：
  //   position.w    = radius（外半径）      endPosition.w = innerRadius
  //   params.x = Kd（diffuse）  params.y = Ks（specular/反射）
  //   denomCone = 1/(innerRadius - radius)，因此内半径必须 ≤ 外半径。
  // 编辑器此前用 `diffuse × 16` + `distance = radius*2` + `decay = 1`，
  // 与游戏的两套归一化分母不同源，这是"同一数值两边亮度不一致"的根因。
  const inner = Math.min(Math.max(unit.innerRadius ?? 0, 0), radius);
  const intensity = Math.max(unit.diffuse ?? 1, 0.05);
  // 锥体半角：游戏以「段长 + 外半径」定义锥体，而不是以半径当距离。
  const coneAngle = Math.min(Math.atan2(radius, Math.max(length, 0.001)), 1.45);
  // 内外半径差 → 柔边：inner 越接近 radius，过渡越硬。
  const penumbra = radius > 0.0001 ? Math.min(Math.max(1 - inner / radius, 0), 1) : 0;

  if (unit.lightType === "Spot") {
    const spot = new THREE.SpotLight(
      rgb.getHex(),
      intensity,
      length,
      coneAngle,
      penumbra,
      1,
    );
    spot.target.position.set(0, length, 0);
    group.add(spot, spot.target);
  } else if (unit.lightType === "Line") {
    // 条形灯 = 沿段长均分的点光；每盏的覆盖半径用外半径（不是段长 × 1.2）。
    const segments = Math.min(Math.max(Math.ceil(length / 8), 2), 4);
    for (let index = 0; index < segments; index += 1) {
      const light = new THREE.PointLight(
        rgb.getHex(),
        intensity / segments,
        radius,
        1,
      );
      light.position.set(0, (length * (index + 0.5)) / segments, 0);
      group.add(light);
    }
  } else {
    group.add(new THREE.PointLight(rgb.getHex(), intensity, radius, 1));
  }
  // 反射/高光（游戏 params.y = Ks）：three.js 没有「每盏灯独立 specular」，
  // 用同位置、半径收缩到 35% 的辅助点光近似高光瓣；强度 = Ks，与 Kd 同量纲。
  // 只在 Ks > 0 时创建，避免给没有反射的灯凭空加光。
  const specular = Math.max(unit.specular ?? 0, 0);
  if (specular > 0) {
    const gloss = new THREE.PointLight(rgb.getHex(), specular, radius * 0.35, 1);
    if (unit.lightType === "Line") {
      gloss.position.set(0, length / 2, 0);
    } else if (unit.lightType === "Spot") {
      gloss.position.set(0, length, 0);
    }
    gloss.name = "__lightSpecular";
    group.add(gloss);
  }
  // 裁切档位（CPU 侧语义，不打包进 shader）：按相机距离决定是否渲染。
  // 档位阈值来自用户实测的游戏画质档（低→高）：Near<10m、Mid<25m、Far<50m、Max<100m。
  group.userData.lightCullDistance = unit.cullDistance ?? "Max";
  group.userData.lightOuterRadius = radius;
  const cullLimit = LIGHT_CULL_DISTANCE[unit.cullDistance ?? "Max"];
  const worldPos = new THREE.Vector3();
  // three.js 每帧对可见对象调用 onBeforeRender：在这里按距离切换**灯本体**的
  // visible（拾取代理不参与，保证编辑期仍可选中/移动）。
  group.onBeforeRender = (_renderer, _scene, camera) => {
    group.getWorldPosition(worldPos);
    const far = camera.position.distanceTo(worldPos) > cullLimit;
    for (const child of group.children) {
      if ((child as { isLight?: boolean }).isLight) child.visible = !far;
    }
  };

  let proxyGeometry: ThreeNamespace.BufferGeometry;
  if (unit.lightType === "Line") {
    proxyGeometry = new THREE.BoxGeometry(1, length, 1);
    proxyGeometry.translate(0, length / 2, 0);
  } else {
    proxyGeometry = new THREE.SphereGeometry(1, 12, 8);
  }
  const proxy = new THREE.Mesh(
    proxyGeometry,
    new THREE.MeshBasicMaterial({
      transparent: true,
      opacity: 0,
      depthWrite: false,
      colorWrite: false,
    }),
  );
  proxy.userData.pickOnly = true;
  group.add(proxy);
  // Game parity: a spot light with a volumetric flag owns a visible cone of
  // light, not just an invisible Three.SpotLight. Keep it after the proxy so
  // the proxy stays the preferred pick target; the beam itself never picks or
  // casts shadows (userData.pickOnly + raycast no-op).
  if (unit.lightType === "Spot" && unit.isVolumetric !== false) {
    const beam = createLightVolumeMesh(THREE, {
      color: rgb,
      length,
      radius: lightRadius(unit),
      intensity: lightVolumeIntensity(unit.diffuse, unit.volumetricStrength),
    });
    group.add(beam.mesh);
  }
  applyTransform(THREE, group, unit.transform);
  return group;
}

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
    object.userData.unitId = unitPlanId(unit);
    object.userData.unitKind = unit.kind;
  }
  return object;
}
