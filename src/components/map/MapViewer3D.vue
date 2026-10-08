<script setup lang="ts">
/**
 * 3D 地图查看器：three.js 分块 LOD 地形（引擎同款 2048m 块 × 16×16：
 * 近 8m/格=游戏精度、中 32m、远 128m；解析法线跨块无缝）+
 * 水面纹理 + 实例化植被 + 地块框/伟工标记 + 栅格道路预览 + 生态资源图层。
 * 相机和图层变化按需渲染；水面可见时以 30fps 更新波纹。
 * 数据源 = map_panel_region_3d（16m/格 高度+地面编码 PNG）。
 * 交互：左键旋转 / 中键或右键平移 / 滚轮缩放（事件就地拦截，不冒泡到页面）。
 */
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import FIcon from "@/components/extensions/FIcon.vue";
import type { Region3DData } from "@/lib/region-map";
import { createTerrainMaterial } from "./terrain-material";
import { createTerrainSkirtGeometry } from "./terrain-skirt";
import { roadPreviewMask } from "./map-fields";
import { createVegetation } from "./map-vegetation";
import { nativeTreeAssets } from "./map-native-assets";
import { createNativeRoads } from "./map-native-roads";
import { createMapWater } from "./map-water";
import { createMapSceneRoot, mapToScene, sceneToMap } from "./map-coordinates";

const props = defineProps<{
  data: Region3DData | null;
  /** 地块框层开关（默认开）。 */
  showPlots?: boolean;
  showVegetation?: boolean;
  showWater?: boolean;
  showRoads?: boolean;
  activeResource?: string | null;
  resourceOpacity?: number;
}>();
const { t, locale } = useI18n();
const localizedLabels: Array<{ sprite: THREE.Sprite; text: () => string; color: string; fontPx: number }> = [];

function localizedSprite(text: () => string, color: string, fontPx = 42): THREE.Sprite {
  const sprite = textSprite(text(), color, fontPx);
  localizedLabels.push({ sprite, text, color, fontPx });
  return sprite;
}

// Refresh only label textures: switching language must preserve the camera and terrain.
watch(locale, () => {
  for (const { sprite, text, color, fontPx } of localizedLabels) {
    const replacement = textSprite(text(), color, fontPx);
    sprite.material.map?.dispose();
    sprite.material.dispose();
    sprite.material = replacement.material;
    sprite.scale.copy(replacement.scale);
  }
  needsRenderFlag = true;
});

const wrap = ref<HTMLDivElement | null>(null);
const ready = ref(false);
const error = ref("");

let renderer: THREE.WebGLRenderer | null = null;
let scene: THREE.Scene | null = null;
let camera: THREE.PerspectiveCamera | null = null;
let controls: OrbitControls | null = null;
let root: THREE.Group | null = null;
let plotGroup: THREE.Group | null = null;
let vegetationGroup: THREE.Group | null = null;
let roadGroup: THREE.Group | null = null;
let waterMesh: THREE.Mesh | null = null;
let setResource: ((kind: string | null, opacity: number) => void) | null = null;
let buildGeneration = 0;
let raf = 0;
let observer: ResizeObserver | null = null;
let needsRenderFlag = true; // 按需渲染：任何视觉变更/尺寸变化都置位

/** 解码 base64 PNG → 像素（无损经 canvas ImageData）。 */
async function decodePng(b64: string): Promise<ImageData> {
  const img = new Image();
  img.src = `data:image/png;base64,${b64}`;
  await img.decode();
  const cv = document.createElement("canvas");
  cv.width = img.width;
  cv.height = img.height;
  const ctx = cv.getContext("2d", { willReadFrequently: true })!;
  ctx.drawImage(img, 0, 0);
  return ctx.getImageData(0, 0, img.width, img.height);
}

/** canvas 文字 Sprite（标注标签）。 */
function textSprite(text: string, color: string, fontPx = 42): THREE.Sprite {
  const cv = document.createElement("canvas");
  const pad = 12;
  const ctx0 = cv.getContext("2d")!;
  ctx0.font = `500 ${fontPx}px system-ui, sans-serif`;
  const w = Math.ceil(ctx0.measureText(text).width) + pad * 2;
  cv.width = w;
  cv.height = fontPx + pad * 2;
  const ctx = cv.getContext("2d")!;
  ctx.font = `500 ${fontPx}px system-ui, sans-serif`;
  ctx.textBaseline = "middle";
  ctx.textAlign = "center";
  ctx.lineWidth = 6;
  ctx.strokeStyle = "rgba(8,12,20,0.9)";
  ctx.strokeText(text, cv.width / 2, cv.height / 2);
  ctx.fillStyle = color;
  ctx.fillText(text, cv.width / 2, cv.height / 2);
  const tex = new THREE.CanvasTexture(cv);
  tex.colorSpace = THREE.SRGBColorSpace;
  const mat = new THREE.SpriteMaterial({ map: tex, depthTest: false, transparent: true });
  const spr = new THREE.Sprite(mat);
  const scale = 260; // 世界米/像素基准
  spr.scale.set((cv.width / cv.height) * scale, scale, 1);
  spr.renderOrder = 10;
  return spr;
}

/** 蓝色尺度标注组（图2式）：主尺线 + 端点刻 + 步进刻度与标签。 */
function buildRuler(
  from: THREE.Vector3,
  to: THREE.Vector3,
  tickDir: THREE.Vector3,
  stepM: number,
  labelOf: (m: number) => string,
): THREE.Group {
  const g = new THREE.Group();
  const mat = new THREE.LineBasicMaterial({
    color: new THREE.Color("#4a9eff"),
    transparent: true,
    opacity: 0.9,
  });
  const dir = to.clone().sub(from);
  const len = dir.length();
  dir.normalize();
  const tick = tickDir.clone().normalize();
  const pts: number[] = [];
  const arm = tick.clone().multiplyScalar(90);
  const a0 = from.clone().add(arm);
  const b0 = to.clone().add(arm);
  pts.push(a0.x, a0.y, a0.z, b0.x, b0.y, b0.z);
  for (const p of [from, to]) {
    const q1 = p.clone().sub(arm);
    pts.push(q1.x, q1.y, q1.z, p.x, p.y, p.z);
    const q2 = p.clone().add(arm);
    pts.push(q2.x, q2.y, q2.z, p.x, p.y, p.z);
  }
  const n = Math.floor(len / stepM);
  for (let i = 0; i <= n; i++) {
    const p = from.clone().add(dir.clone().multiplyScalar(i * stepM));
    const q = p.clone().add(tick.clone().multiplyScalar(150));
    pts.push(p.x, p.y, p.z, q.x, q.y, q.z);
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
  g.add(new THREE.LineSegments(geo, mat));
  for (let i = 0; i <= n; i++) {
    if (i > 0 && i < n && i % 2 !== 0) continue;
    const p = from.clone().add(dir.clone().multiplyScalar(i * stepM));
    const label = textSprite(labelOf(i * stepM), "#7cbcff", 40);
    label.position.copy(p.clone().add(tick.clone().multiplyScalar(320)));
    g.add(label);
  }
  return g;
}

// ══ 分块 LOD 地形 ══
// 引擎同款：2048m 块 × 16×16；LOD0 = 8m/格（游戏精度）、LOD1 = 32m、LOD2 = 128m。
// 法线按世界高度场解析求取（跨块无缝）；按需渲染 + 每帧限量重建。
interface Chunk {
  cx: number; // 块索引 0..15
  cy: number;
  mesh: THREE.Mesh | null;
  skirt: THREE.Mesh | null;
  lod: number; // -1 = 未建
}

function makeTerrainEngine(
  data: Region3DData,
  raw: Uint16Array,
  groundPx: ImageData,
  detailPixels: Partial<Record<"dirt" | "grass" | "cliff" | "sand", ImageData>>,
  cameraRef: () => THREE.PerspectiveCamera | null,
  needsRender: () => void,
) {
  const S = Math.round(Math.sqrt(raw.length));
  const dataMpp = data.metersPerPixel > 0 ? data.metersPerPixel : 16;
  const hDiv = data.heightDiv || 32;
  const hBias = data.heightBias ?? -1024;
  const waterZ = data.waterZ;

  /** 世界坐标 → 高度（raw 场双线性，16m/格）。 */
  function heightWorld(wx: number, wy: number): number {
    const fx = Math.min(S - 1, Math.max(0, (wx - ORGX) / dataMpp));
    const fy = Math.min(S - 1, Math.max(0, (wy - ORGY) / dataMpp));
    const x0 = Math.floor(fx);
    const y0 = Math.floor(fy);
    const x1 = Math.min(S - 1, x0 + 1);
    const y1 = Math.min(S - 1, y0 + 1);
    const tx = fx - x0;
    const ty = fy - y0;
    const h =
      (raw[y0 * S + x0] * (1 - tx) + raw[y0 * S + x1] * tx) * (1 - ty) +
      (raw[y1 * S + x0] * (1 - tx) + raw[y1 * S + x1] * tx) * ty;
    return h / hDiv + hBias;
  }

  const CHUNK = 2048;
  const ORGX = data.originWorld[0];
  const ORGY = data.originWorld[1];
  const SPAN = data.size * data.metersPerPixel;
  const N = Math.max(1, Math.round(SPAN / CHUNK)); // 后端已对齐 2048 格
  const LOD_VERTS = [257, 65, 17]; // 每块边顶点数（8m/32m/128m 格）
  const idxCache = new Map<number, Uint32Array>();

  function indexFor(verts: number): Uint32Array {
    const cached = idxCache.get(verts);
    if (cached) return cached;
    const n = verts - 1;
    const idx = new Uint32Array(n * n * 6);
    let ii = 0;
    for (let j = 0; j < n; j++) {
      for (let i = 0; i < n; i++) {
        const a = j * verts + i;
        idx[ii++] = a; idx[ii++] = a + verts; idx[ii++] = a + 1;
        idx[ii++] = a + 1; idx[ii++] = a + verts; idx[ii++] = a + verts + 1;
      }
    }
    idxCache.set(verts, idx);
    return idx;
  }

  function buildChunk(chunk: Chunk, lod: number): void {
    const verts = LOD_VERTS[lod];
    const step = CHUNK / (verts - 1);
    const wx0 = ORGX + chunk.cx * CHUNK;
    const wy0 = ORGY + chunk.cy * CHUNK;
    const vcount = verts * verts;
    const pos = new Float32Array(vcount * 3);
    const nor = new Float32Array(vcount * 3);

    let vi = 0;
    for (let j = 0; j < verts; j++) {
      const wy = wy0 + j * step;
      for (let i = 0; i < verts; i++) {
        const wx = wx0 + i * step;
        const z = heightWorld(wx, wy);
        pos[vi * 3] = wx;
        pos[vi * 3 + 1] = z;
        pos[vi * 3 + 2] = wy;
        // 解析法线（世界高度场中心差分，跨块无缝）
        const zx = (heightWorld(wx + dataMpp, wy) - heightWorld(wx - dataMpp, wy)) / (2 * dataMpp);
        const zy = (heightWorld(wx, wy + dataMpp) - heightWorld(wx, wy - dataMpp)) / (2 * dataMpp);
        const inv = 1 / Math.hypot(zx, 1, zy);
        nor[vi * 3] = -zx * inv;
        nor[vi * 3 + 1] = inv;
        nor[vi * 3 + 2] = -zy * inv;
        vi++;
      }
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    geo.setAttribute("normal", new THREE.BufferAttribute(nor, 3));

    geo.setIndex(new THREE.BufferAttribute(indexFor(verts), 1));
    geo.boundingSphere = new THREE.Sphere(
      new THREE.Vector3(wx0 + CHUNK / 2, 0, wy0 + CHUNK / 2),
      CHUNK * 0.75 + 2048,
    );
    const mat =
      (chunk.mesh?.material as THREE.Material) ?? terrainMaterial;
    if (chunk.mesh) {
      chunk.mesh.geometry.dispose();
      chunk.mesh.geometry = geo;
    } else {
      chunk.mesh = new THREE.Mesh(geo, mat);
      chunk.mesh.matrixAutoUpdate = false;
      group.add(chunk.mesh);
    }
    if (chunk.cx === 0 || chunk.cy === 0 || chunk.cx === N - 1 || chunk.cy === N - 1) {
      const skirtGeo = createTerrainSkirtGeometry(pos, verts, {
        north: chunk.cy === 0, east: chunk.cx === N - 1,
        south: chunk.cy === N - 1, west: chunk.cx === 0,
      }, Math.min(data.heightBias, waterZ - 60));
      if (chunk.skirt) {
        chunk.skirt.geometry.dispose();
        chunk.skirt.geometry = skirtGeo;
      } else {
        chunk.skirt = new THREE.Mesh(skirtGeo, skirtMaterial);
        group.add(chunk.skirt);
      }
    }
    chunk.lod = lod;
  }

  const chunks: Chunk[] = [];
  const group = new THREE.Group();
  const terrainMaterial = createTerrainMaterial(data, raw, groundPx.data, detailPixels);
  const skirtMaterial = new THREE.MeshLambertMaterial({ vertexColors: true });
  for (let cy = 0; cy < N; cy++) {
    for (let cx = 0; cx < N; cx++) {
      chunks.push({ cx, cy, mesh: null, skirt: null, lod: -1 });
    }
  }

  const lodQueue: Chunk[] = [];
  const centerV = new THREE.Vector3();
  /** LOD0 半径内块数 ≤ ~5，全图 LOD2 仅 1.3 万顶点。 */
  function desiredLod(chunk: Chunk): number {
    centerV.set(
      ORGX + chunk.cx * CHUNK + CHUNK / 2,
      0,
      ORGY + chunk.cy * CHUNK + CHUNK / 2,
    );
    const cam = cameraRef();
    // Chunk centers are map-local; the camera is in the corrected Three world.
    centerV.z = -centerV.z;
    const d = cam ? cam.position.distanceTo(centerV) : 1e9;
    if (d < 5200) return 0;
    if (d < 14000) return 1;
    return 2;
  }

  function updateLods(): void {
    lodQueue.length = 0;
    for (const ch of chunks) {
      const want = desiredLod(ch);
      if (want !== ch.lod) lodQueue.push(ch);
    }
  }

  /** 每帧限量处理重建队列（防卡顿尖峰）。 */
  function processQueue(budget: number): boolean {
    if (!lodQueue.length) return false;
    let n = 0;
    while (lodQueue.length && n < budget) {
      const ch = lodQueue.shift()!;
      buildChunk(ch, desiredLod(ch));
      n++;
    }
    needsRender();
    return true;
  }

  function dispose(): void {
    for (const ch of chunks) {
      ch.mesh?.geometry.dispose();
      ch.skirt?.geometry.dispose();
    }
    terrainMaterial.dispose();
    skirtMaterial.dispose();
    group.clear();
    chunks.length = 0;
    idxCache.clear();
  }

  return { group, updateLods, processQueue, dispose, heightWorld, setResource: terrainMaterial.setResource };
}

async function build(data: Region3DData) {
  const generation = ++buildGeneration;
  disposeScene();
  error.value = "";
  const container = wrap.value;
  if (!container) return;
  ready.value = false;

  // ── 数据解码 ──
  let heightPx: ImageData;
  let groundPx: ImageData;
  let foamPixels: ImageData | undefined;
  let treePixels: ImageData[];
  const roadPixels: Record<string, ImageData> = {};
  const detailPixels: Partial<Record<"dirt" | "grass" | "cliff" | "sand", ImageData>> = {};
  try {
    [heightPx, groundPx] = await Promise.all([
      decodePng(data.heightPngBase64),
      decodePng(data.groundPngBase64),
    ]);
    await Promise.all((["dirt", "grass", "cliff", "sand"] as const).map(async (slot) => {
      const png = data.terrainTextures?.[slot];
      if (png) detailPixels[slot] = await decodePng(png);
    }));
    if (data.waterFoamPngBase64) foamPixels = await decodePng(data.waterFoamPngBase64);
    treePixels = await Promise.all((data.forestModels ?? []).map(model => decodePng(model.diffusePngBase64)));
    await Promise.all(Object.entries(data.roadAssets?.textures ?? {}).map(async ([id,png]) => {roadPixels[id]=await decodePng(png);}));
    await Promise.all(Object.entries(data.roadAssets?.models ?? {}).map(async ([id,model]) => {roadPixels[`model:${id}`]=await decodePng(model.diffusePngBase64);}));
  } catch (e) {
    if (generation !== buildGeneration) return;
    error.value = String(e);
    return;
  }
  if (generation !== buildGeneration) return;
  const S = heightPx.width; // 以解码后实际尺寸为准（防 DTO 字段不齐产生 NaN 采样）
  const raw = new Uint16Array(S * S);
  for (let i = 0; i < S * S; i++) raw[i] = heightPx.data[i * 4] * 256 + heightPx.data[i * 4 + 1];

  // ── 场景基座 ──
  scene = new THREE.Scene();
  scene.background = new THREE.Color("#0b0f17");
  scene.fog = new THREE.Fog("#0b0f17", 42000, 90000);
  camera = new THREE.PerspectiveCamera(45, 1, 20, 200000);
  const ORG = data.originWorld;
  const SPAN = data.size * data.metersPerPixel;
  const CTR = mapToScene(ORG[0] + SPAN / 2, ORG[1] + SPAN / 2, 0);
  camera.position.copy(mapToScene(ORG[0] + SPAN * 0.78, ORG[1] + SPAN * 1.8, SPAN * 1.05));
  renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.75));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  container.appendChild(renderer.domElement);
  // 中键自动滚屏 / 右键菜单 / 滚轮页面滚动：就地拦截，不冒泡到页面
  const el = renderer.domElement;
  el.addEventListener("pointerdown", (e) => {
    if (e.button === 1 || e.button === 2) e.preventDefault();
  });
  el.addEventListener("contextmenu", (e) => e.preventDefault());
  el.addEventListener("wheel", (e) => e.preventDefault(), { passive: false });
  controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = 0.08;
  controls.maxPolarAngle = Math.PI * 0.49;
  controls.minDistance = 800;
  controls.maxDistance = Math.max(30000, SPAN * 2.2);
  controls.target.copy(CTR);
  controls.saveState();
  controls.mouseButtons = {
    LEFT: THREE.MOUSE.ROTATE,
    MIDDLE: THREE.MOUSE.PAN,
    RIGHT: THREE.MOUSE.PAN,
  };

  const sun = new THREE.DirectionalLight(0xffffff, 1.35);
  sun.position.copy(mapToScene(9000, 8000, 22000));
  scene.add(sun);
  scene.add(new THREE.AmbientLight(0xffffff, 0.62));
  scene.add(new THREE.HemisphereLight(0x9db8d8, 0x30281c, 0.5));

  root = createMapSceneRoot();
  scene.add(root);

  // ── 分块 LOD 地形 ──
  const terrain = makeTerrainEngine(
    data,
    raw,
    groundPx,
    detailPixels,
    () => camera,
    () => {
      needsRenderFlag = true;
    },
  );
  root.add(terrain.group);
  setResource = terrain.setResource;
  setResource(props.activeResource ?? null, props.resourceOpacity ?? 0.75);
  const roadMask = roadPreviewMask(groundPx.data, S);

  terrain.updateLods();
  terrain.processQueue(256); // 首帧一次成型（全图 LOD2 ≈ 1.3 万顶点）

  // ── 水面 ──
  const water = createMapWater(data, heightPx, foamPixels);
  waterMesh = water.mesh;
  const hasWater = raw.some((value) => value / data.heightDiv + data.heightBias < data.waterZ);
  waterMesh.userData.hasWater = hasWater;
  waterMesh.visible = hasWater && props.showWater !== false && !props.activeResource;
  root.add(waterMesh);

  // ── 地面网格（图2式底格，水下）──
  const grid = new THREE.GridHelper(
    SPAN,
    Math.max(1, Math.round(SPAN / 2048)),
    0x31415c,
    0x223049,
  );
  (grid.material as THREE.Material).transparent = true;
  (grid.material as THREE.Material).opacity = 0.55;
  grid.position.set(ORG[0] + SPAN / 2, data.waterZ - 6, ORG[1] + SPAN / 2);
  root.add(grid);

  const roads = data.saveLayers && data.roadAssets ? createNativeRoads(data, roadPixels, terrain.heightWorld) : new THREE.Group();
  roads.visible = props.showRoads !== false && !props.activeResource;
  roadGroup = roads;
  root.add(roads);
  vegetationGroup = createVegetation(data, groundPx, roadMask, terrain.heightWorld,
    nativeTreeAssets(data.forestModels ?? [], treePixels, renderer));
  vegetationGroup.visible = props.showVegetation !== false && !props.activeResource;
  root.add(vegetationGroup);

  // ── 地块框 + 伟工标记 + 名字标签 ──
  plotGroup = new THREE.Group();
  plotGroup.visible = props.showPlots !== false;
  const yellow = new THREE.LineBasicMaterial({
    color: 0xf5c518,
    transparent: true,
    opacity: 0.95,
    depthTest: false, // 地块框永不被地形遮挡
  });
  const magenta = new THREE.LineBasicMaterial({
    color: 0xe040e0,
    transparent: true,
    opacity: 0.95,
    depthTest: false,
  });
  for (const p of data.plots) {
    const isGw = p.kind === "greatwork";
    const lift = isGw ? 30 : 10;
    const z = Math.max(p.z, data.waterZ) + lift;
    let outline: THREE.Line;
    if (isGw) {
      // 伟工 = 圆形用地框（半径 768m，对齐游戏内椭圆用地观感）
      const pts: number[] = [];
      const R = 768;
      for (let i = 0; i < 64; i++) {
        const a = (i / 64) * Math.PI * 2;
        pts.push(p.x + Math.cos(a) * R, z, p.y + Math.sin(a) * R);
      }
      const lg = new THREE.BufferGeometry();
      lg.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
      outline = new THREE.LineLoop(lg, magenta);
    } else {
      const sizeHalf = 1024;
      const pts: number[] = [];
      const corners: [number, number][] = [
        [p.x - sizeHalf, p.y - sizeHalf],
        [p.x + sizeHalf, p.y - sizeHalf],
        [p.x + sizeHalf, p.y + sizeHalf],
        [p.x - sizeHalf, p.y + sizeHalf],
      ];
      for (let i = 0; i < 4; i++) {
        const [ax, ay] = corners[i];
        const [bx, by] = corners[(i + 1) % 4];
        pts.push(ax, z, ay, bx, z, by);
      }
      const lg = new THREE.BufferGeometry();
      lg.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
      outline = new THREE.LineSegments(lg, yellow);
    }
    outline.renderOrder = 5;
    plotGroup.add(outline);
    if (isGw) {
      const gem = new THREE.Mesh(
        new THREE.OctahedronGeometry(140),
        new THREE.MeshLambertMaterial({
          color: 0xe040e0,
          depthTest: false,
        }),
      );
      gem.renderOrder = 6;
      gem.position.set(p.x, z + 220, p.y);
      plotGroup.add(gem);
    }
    const label = localizedSprite(
      () => isGw ? t("studio.map.greatWorkSite")
        : locale.value.startsWith("zh") ? (p.name ?? p.nameEn ?? p.uid)
          : (p.nameEn ?? t("studio.map.citySite", { id: p.uid })),
      isGw ? "#f08cf0" : "#ffe9a8",
      38,
    );
    label.position.set(p.x, z + (isGw ? 430 : 180), p.y);
    plotGroup.add(label);
  }
  root.add(plotGroup);

  // ── 尺度标注（图2式蓝尺）：南缘 X 尺 + 西缘 Y 尺 + 角上高尺 ──
  const off = 1500;
  const rulerY = data.waterZ + 20;
  root.add(
    buildRuler(
      new THREE.Vector3(ORG[0], rulerY, ORG[1] + SPAN + off),
      new THREE.Vector3(ORG[0] + SPAN, rulerY, ORG[1] + SPAN + off),
      new THREE.Vector3(0, 0, 1),
      2048,
      (m) => `${Math.round(ORG[0] + m)}m`,
    ),
  );
  root.add(
    buildRuler(
      new THREE.Vector3(ORG[0] - off, rulerY, ORG[1]),
      new THREE.Vector3(ORG[0] - off, rulerY, ORG[1] + SPAN),
      new THREE.Vector3(-1, 0, 0),
      2048,
      (m) => `${Math.round(ORG[1] + m)}m`,
    ),
  );
  const hDivL2 = data.heightDiv || 32;
  const hBiasL2 = data.heightBias ?? -1024;
  let zMax = data.waterZ;
  for (let i = 0; i < raw.length; i += 97) {
    const z = raw[i] / hDivL2 + hBiasL2;
    if (z > zMax) zMax = z;
  }
  const hRuler = buildRuler(
    new THREE.Vector3(ORG[0] - off, data.waterZ, ORG[1] + SPAN + off),
    new THREE.Vector3(ORG[0] - off, zMax, ORG[1] + SPAN + off),
    new THREE.Vector3(0, 1, 0),
    Math.max(250, Math.round((zMax - data.waterZ) / 4 / 50) * 50),
    (m) => `${m}m`,
  );
  const hLabel = localizedSprite(() => t("studio.map.heightMarker", { height: Math.round(zMax - data.waterZ) }), "#7cbcff", 46);
  hLabel.position.set(ORG[0] - off, (data.waterZ + zMax) / 2 + 200, ORG[1] + SPAN + off + 380);
  hRuler.add(hLabel);
  root.add(hRuler);

  // ── 按需渲染：相机动/重建队列动才进 GPU；LOD 升级限量 6 块/帧 ──
  controls.addEventListener("change", () => {
    needsRenderFlag = true;
    terrain.updateLods();
  });
  let lastLodTs = 0;
  let lastWaterTs = 0;
  const loop = () => {
    raf = requestAnimationFrame(loop);
    if (!renderer || !scene || !camera || !controls) return;
    const moved = controls.update();
    // 地形碰撞：相机不得低于脚下地形 +30m（双线性采样含裁剪窗外沿）
    {
      const mapCamera = sceneToMap(camera.position);
      const minY = terrain.heightWorld(mapCamera.x, mapCamera.y) + 30;
      if (camera.position.y < minY) {
        camera.position.y = minY;
        needsRenderFlag = true;
      }
    }
    const now = performance.now();
    if (water.mesh.visible && !document.hidden && now - lastWaterTs >= 1000 / 30) {
      lastWaterTs = now;
      water.update(now / 1000);
      needsRenderFlag = true;
    }
    if (now - lastLodTs > 120) {
      lastLodTs = now;
      terrain.processQueue(6);
    }
    if (moved || needsRenderFlag) {
      needsRenderFlag = false;
      renderer.render(scene, camera);
    }
  };
  onResize();
  loop();
  ready.value = true;
}

function onResize() {
  if (!renderer || !camera || !wrap.value) return;
  const w = wrap.value.clientWidth || 1;
  const h = wrap.value.clientHeight || 1;
  renderer.setSize(w, h, false);
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
  if (controls) controls.update();
  needsRenderFlag = true; // 首帧/尺寸变化后必须有一次重绘，否则黑屏到下次交互
}

function disposeScene() {
  cancelAnimationFrame(raf);
  controls?.dispose();
  controls = null;
  renderer?.dispose();
  renderer?.domElement.remove();
  renderer = null;
  setResource = null;
  waterMesh = null;
  roadGroup = null;
  vegetationGroup = null;
  localizedLabels.length = 0;
  plotGroup = null;
  if (root) {
    const geometries = new Set<THREE.BufferGeometry>();
    const materials = new Set<THREE.Material>();
    const labelTextures = new Set<THREE.Texture>();
    root.traverse((o) => {
      const m = o as THREE.Mesh;
      if (m.geometry) geometries.add(m.geometry);
      const mat = m.material as THREE.Material | THREE.Material[] | undefined;
      if (Array.isArray(mat)) mat.forEach((x) => materials.add(x));
      else if (mat) materials.add(mat);
      if ((o as THREE.Sprite).isSprite) {
        const map = (o as THREE.Sprite).material.map;
        if (map) labelTextures.add(map);
      }
      if ((o as THREE.InstancedMesh).isInstancedMesh) (o as THREE.InstancedMesh).dispose();
    });
    geometries.forEach((g) => g.dispose());
    materials.forEach((m) => m.dispose());
    labelTextures.forEach((texture) => texture.dispose());
    scene?.remove(root);
    root = null;
  }
}

onMounted(() => {
  observer = new ResizeObserver(onResize);
  if (wrap.value) observer.observe(wrap.value);
  if (props.data) void build(props.data);
});

onBeforeUnmount(() => {
  buildGeneration++;
  observer?.disconnect();
  disposeScene();
  scene = null;
});

watch(
  () => props.data,
  (d) => {
    if (d) void build(d);
    else { buildGeneration++; ready.value = false; disposeScene(); }
  },
);
watch(
  () => props.showPlots,
  (v) => {
    if (plotGroup) plotGroup.visible = v !== false;
    needsRenderFlag = true;
  },
);
watch(() => [props.showVegetation, props.showWater, props.showRoads, props.activeResource, props.resourceOpacity], () => {
  const resourceView = Boolean(props.activeResource);
  if (vegetationGroup) vegetationGroup.visible = props.showVegetation !== false && !resourceView;
  if (roadGroup) roadGroup.visible = props.showRoads !== false && !resourceView;
  if (waterMesh) waterMesh.visible = waterMesh.userData.hasWater && props.showWater !== false && !resourceView;
  setResource?.(props.activeResource ?? null, props.resourceOpacity ?? 0.75);
  needsRenderFlag = true;
});
</script>

<template>
  <div ref="wrap" class="viewer3d">
    <div v-if="!data && !error" class="ph"></div>
    <div v-if="error" class="err">{{ error }}</div>
    <button
      v-if="ready"
      type="button"
      class="reset-btn"
      :title="t('studio.map.resetView')"
      @click="
        () => {
          controls?.reset();
        }
      "
    >
      <FIcon name="RotateCcw" :size="14" aria-label="" />
    </button>
    <div v-if="ready" class="hud">
      <span class="hud-item">{{ t('studio.map.mapExtent', { size: data ? (data.size * data.metersPerPixel / 1000).toFixed(1) : '?' }) }}</span>
      <span class="hud-item">{{ t('studio.map.gridDetail') }}</span>
    </div>
  </div>
</template>

<style scoped>
.viewer3d {
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 420px;
  background: var(--surface);
  overflow: hidden;
}
.viewer3d :deep(canvas) {
  position: absolute;
  inset: 0;
  display: block;
}
.ph {
  position: absolute;
  inset: 0;
}
.err {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 1rem;
  font-size: 0.75rem;
  color: var(--destruct, #b91c1c);
}
.reset-btn {
  position: absolute;
  top: 8px;
  right: 8px;
  z-index: 2;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  min-height: 28px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--surface) 85%, transparent);
  color: var(--muted-foreground);
  cursor: pointer;
}
.reset-btn:hover {
  color: var(--foreground);
  background: var(--surface-hover);
}
.hud {
  position: absolute;
  bottom: 8px;
  right: 8px;
  z-index: 2;
  display: flex;
  gap: 6px;
}
.hud-item {
  padding: 2px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--surface) 85%, transparent);
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  color: var(--muted-foreground);
}
</style>
