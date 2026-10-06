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

/** unit id → 数字 seed（FNV-1）：小人外观确定性随机用。 */
function spawnerSeed(unit: LotUnitDto): number {
  const id = unitId(unit);
  let hash = 0x811c9dc5;
  for (let i = 0; i < id.length; i += 1) {
    hash ^= id.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
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

/** 确定性伪随机（mulberry32）：小人肤色/衣色/身高抖动用，seed 派生自
 * unit id——同一刷新点恒同一小人。 */
function mulberry32(seed: number): () => number {  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = Math.imul(t ^ (t >>> 7), 61 | t) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** 小人肤色/衣色调色板（观感校准用近似；引擎真值在外观表调色板，未本地化）。 */
const SIM_SKIN_TONES = [0xe8b89a, 0xd9a077, 0xb97a50, 0x8a5a38, 0x6b4226];
const SIM_OUTFIT_COLORS = [
  0xc81e1e, 0x1e40c8, 0x20c050, 0xf0a000, 0x8848c8, 0x20b0b0, 0xe8e8e8,
  0x303038, 0xc84890,
];
const SIM_PANTS_COLORS = [0x303040, 0x40485c, 0x242428, 0x54423a];

/**
 * 小人占位（spawner 渲染，2026-10-06）：风格化人形——双腿+躯干+双臂+头球，
 * 站姿，身高 ~1.75m ± 抖动。肤色/衣色按 seed 确定性取自调色板。
 *
 * 引擎口径（props 文档 §3，source-tree 实证）：spawner = cUnitModel 的
 * 具名位置锚点（0x0E1BAC61 键列 instance=锚点名 id + 0x0E1BAC62 变换列 →
 * mUnitLocations），agent 在此生成（GetUnitLocation = 模型变换∘锚点变换，
 * BuildUnitJSValue 按名字哈希硬编码查询）；小人外观由
 * cGraphicsInstancedSim 外观表（0x0CBD25C1-CB：heads/bodies/outfits 及
 * Max + 缩放域）按 randomBits 取模解析（body≤3/head≤80/outfit 调色板，
 * 城市级全局模型目录，本地包未见）——故 PE 用占位人形而非真模型。
 */
function buildSimFigure(THREE: Three, seed: number): ThreeNamespace.Object3D {  const rand = mulberry32(seed);
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
  // 双腿（裤色）：r0.09 h0.78，站距 0.22
  addLimb(new THREE.CylinderGeometry(0.085, 0.1, 0.78, 10), pants, 0.39, -0.11);
  addLimb(new THREE.CylinderGeometry(0.085, 0.1, 0.78, 10), pants, 0.39, 0.11);
  // 躯干（衣色）：肩宽收腰
  addLimb(new THREE.CylinderGeometry(0.17, 0.21, 0.62, 12), outfit, 1.09);
  // 双臂（衣色）：垂放体侧
  addLimb(new THREE.CylinderGeometry(0.055, 0.065, 0.58, 8), outfit, 1.06, -0.27);
  addLimb(new THREE.CylinderGeometry(0.055, 0.065, 0.58, 8), outfit, 1.06, 0.27);
  // 头（肤色）
  addLimb(new THREE.SphereGeometry(0.155, 14, 12), skin, 1.62);
  group.scale.setScalar(jitter);
  return group;
}

/**
 * spawner 占位人形（精细模式真小人资产缺席时的降级）：风格化人形 + 单元
 * 变换（内部 applyTransform，含位置/朝向——真小人通道外的兜底必须自带
 * 变换，否则全堆在原点）。
 */
export function buildSpawnerPlaceholder(
  THREE: Three,
  unit: LotUnitDto,
): ThreeNamespace.Object3D {
  const figure = buildSimFigure(THREE, spawnerSeed(unit));
  // pathPoint 无 transform（位置由 point 承载），联合收窄
  if (unit.kind !== "pathPoint") {
    applyTransform(THREE, figure, unit.transform);
  }
  return figure;
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
 * 原"锥上序号 billboard"已撤（2026-10-06 用户指令）：场景内常驻小标签牌
 * 视觉噪声大，label 职责移交 hover/选中描边框的左上角标签（Viewport
 * 拾取 overlay）。
 */
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
 * 三种光源**本体完全不可见**（光源球体/灯带模型一律隐藏，避免遮挡精细模型），
 * 仅留透明拾取代理供选择；强度不随环境亮度滑条缩放（夜间灯依然亮）。
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
  const intensity = Math.max(unit.diffuse ?? 1, 0.05) * 16;

  if (unit.lightType === "Spot") {
    const angle = Math.min(Math.atan2(radius, length), 1.45);
    const spot = new THREE.SpotLight(rgb.getHex(), intensity, radius * 2, angle, 0.5, 1);
    spot.target.position.set(0, length, 0);
    group.add(spot, spot.target);
  } else if (unit.lightType === "Line") {
    // 沿灯带（局部 +Y 自原点延伸）均匀布 N 个小范围点光，近似条形氛围照明；
    // 上限 4——前向渲染片元成本随光源数线性涨，灯带多的大地块靠总预算裁剪兜底
    const segments = Math.min(Math.max(Math.ceil(length / 8), 2), 4);
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

  // 不可见拾取代理（透明不写深度；Raycaster 不过滤透明对象）。
  // 注意：光源 unit 在精细模式下只贡献照明，灯位不渲染任何可见几何
  // （用户 2026-09-19 澄清：打光光源本身不应被看见；此前加的发光
  // 灯球/灯管属方向性错误，已撤）。
  let proxyGeometry: ThreeNamespace.BufferGeometry;
  if (unit.lightType === "Line") {
    proxyGeometry = new THREE.BoxGeometry(2, length, 2);
    proxyGeometry.translate(0, length / 2, 0);
  } else if (unit.lightType === "Spot") {
    proxyGeometry = new THREE.CylinderGeometry(
      Math.max(radius * 0.5, 1),
      Math.max(radius * 0.5, 1),
      length,
      8,
      1,
      true,
    );
    proxyGeometry.translate(0, length / 2, 0);
  } else {
    proxyGeometry = new THREE.SphereGeometry(radius, 8, 6);
  }
  group.add(
    new THREE.Mesh(
      proxyGeometry,
      new THREE.MeshBasicMaterial({ transparent: true, opacity: 0, depthWrite: false }),
    ),
  );
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
      // 默认模式 = 蓝色标记锥（真小人/占位人形仅精细模式，见 Viewport）
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
