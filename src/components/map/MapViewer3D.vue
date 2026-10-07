<script setup lang="ts">
/**
 * 3D 地图查看器：three.js 分块 LOD 地形（引擎同款 2048m 块 × 16×16：
 * 近 8m/格=游戏精度、中 32m、远 128m；解析法线跨块无缝）+
 * 全局水位面 + 地块框/伟工标记（本地化名）+ 路网 ribbon（跨水=桥面+桥墩）+
 * 蓝色尺度标注（图2式）。按需渲染：相机静止不进 GPU。
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

const props = defineProps<{
  data: Region3DData | null;
  /** 地块框层开关（默认开）。 */
  showPlots?: boolean;
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
let raf = 0;
let observer: ResizeObserver | null = null;
let needsRenderFlag = true; // 按需渲染：任何视觉变更/尺寸变化都置位

const HALF = 16384;

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

  return { group, updateLods, processQueue, dispose, heightWorld };
}

async function build(data: Region3DData) {
  disposeScene();
  error.value = "";
  const container = wrap.value;
  if (!container) return;
  ready.value = false;

  // ── 数据解码 ──
  let heightPx: ImageData;
  let groundPx: ImageData;
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
  } catch (e) {
    error.value = String(e);
    return;
  }
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
  const CTR = new THREE.Vector3(ORG[0] + SPAN / 2, 0, ORG[1] + SPAN / 2);
  camera.position.set(CTR.x + SPAN * 0.28, CTR.y + SPAN * 1.05, CTR.z + SPAN * 1.3);
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
  controls.mouseButtons = {
    LEFT: THREE.MOUSE.ROTATE,
    MIDDLE: THREE.MOUSE.PAN,
    RIGHT: THREE.MOUSE.PAN,
  };

  const sun = new THREE.DirectionalLight(0xffffff, 1.35);
  sun.position.set(9000, 22000, 8000);
  scene.add(sun);
  scene.add(new THREE.AmbientLight(0xffffff, 0.62));
  scene.add(new THREE.HemisphereLight(0x9db8d8, 0x30281c, 0.5));

  root = new THREE.Group();
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

  terrain.updateLods();
  terrain.processQueue(256); // 首帧一次成型（全图 LOD2 ≈ 1.3 万顶点）

  // ── 水面 ──
  const water = new THREE.Mesh(
    new THREE.PlaneGeometry(SPAN, SPAN),
    new THREE.MeshLambertMaterial({
      color: "#2b5c93",
      transparent: true,
      opacity: 0.82,
    }),
  );
  water.rotation.x = -Math.PI / 2;
  water.position.set(CTR.x, data.waterZ, CTR.z);
  root.add(water);

  // ── 地面网格（图2式底格，水下）──
  const grid = new THREE.GridHelper(
    SPAN,
    Math.max(1, Math.round(SPAN / 2048)),
    0x31415c,
    0x223049,
  );
  (grid.material as THREE.Material).transparent = true;
  (grid.material as THREE.Material).opacity = 0.55;
  grid.position.set(CTR.x, data.waterZ - 6, CTR.z);
  root.add(grid);

  // ── 路网（ED b0==0 mask：16m 格链提取 → Chaikin 平滑 → 平滑路带；跨水=桥面+桥墩）──
  const rp: number[] = [];
  const rc: number[] = [];
  const ri: number[] = [];
  const pillarMats: THREE.Matrix4[] = [];
  const cells = S; // 16m/格
  const cellM = SPAN / cells;
  const hDivL = data.heightDiv || 32;
  const hBiasL = data.heightBias ?? -1024;
  const isRoad = (x: number, y: number): boolean =>
    x >= 0 && y >= 0 && x < cells && y < cells && groundPx.data[(y * S + x) * 4 + 2] === 0;
  // 桥墩：水下路段每 4 格立一根（从水下到桥面）
  for (let y = 0; y < cells; y++) {
    for (let x = 0; x < cells; x += 4) {
      if (!isRoad(x, y)) continue;
      const terZ = raw[y * S + x] / hDivL + hBiasL;
      if (terZ >= data.waterZ) continue;
      const deckZ = Math.max(terZ + 4, data.waterZ + 9);
      const bottom = data.waterZ - 14;
      const h = deckZ - bottom;
      pillarMats.push(
        new THREE.Matrix4().compose(
          new THREE.Vector3(ORG[0] + x * cellM + cellM / 2, bottom + h / 2, ORG[1] + y * cellM + cellM / 2),
          new THREE.Quaternion(),
          new THREE.Vector3(10, h, 10),
        ),
      );
    }
  }
  // 1) 链提取（8 邻接走带，端点优先、方向延续；环路/交叉口由兜底遍历收尾）
  const DIRS8: [number, number][] = [
    [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1], [1, -1],
  ];
  const visited = new Uint8Array(cells * cells);
  const nbrs = (x: number, y: number): [number, number][] => {
    const out: [number, number][] = [];
    for (const [dx, dy] of DIRS8) {
      const nx = x + dx;
      const ny = y + dy;
      if (isRoad(nx, ny) && !visited[ny * cells + nx]) out.push([nx, ny]);
    }
    return out;
  };
  const walkChain = (sx: number, sy: number): [number, number][] => {
    const chain: [number, number][] = [[sx, sy]];
    visited[sy * cells + sx] = 1;
    for (;;) {
      const [cx, cy] = chain[chain.length - 1];
      const cands = nbrs(cx, cy);
      if (!cands.length) break;
      let best = cands[0];
      if (chain.length >= 2) {
        const [qx, qy] = chain[chain.length - 2];
        const dx = cx - qx;
        const dy = cy - qy;
        let bestDot = -Infinity;
        for (const c of cands) {
          const d = (c[0] - cx) * dx + (c[1] - cy) * dy;
          if (d > bestDot) {
            bestDot = d;
            best = c;
          }
        }
      }
      chain.push(best);
      visited[best[1] * cells + best[0]] = 1;
      if (chain.length > 20000) break;
    }
    return chain;
  };
  const chains: [number, number][][] = [];
  for (let y = 0; y < cells; y++) {
    for (let x = 0; x < cells; x++) {
      if (isRoad(x, y) && !visited[y * cells + x] && nbrs(x, y).length === 1) {
        chains.push(walkChain(x, y));
      }
    }
  }
  for (let y = 0; y < cells; y++) {
    for (let x = 0; x < cells; x++) {
      if (isRoad(x, y) && !visited[y * cells + x]) chains.push(walkChain(x, y));
    }
  }
  // 2) Chaikin 平滑 ×2（16m→4m 点距）→ 沿曲线铺路带
  const roadY = (wx: number, wy: number): number => {
    const ter = terrain.heightWorld(wx, wy);
    return Math.max(ter + 2.5, data.waterZ + 9);
  };
  const HW = 14; // 半宽（路宽 28m）
  let base = 0;
  for (const chain of chains) {
    if (chain.length < 2) continue;
    let pts: [number, number][] = chain.map(([x, y]) => [
      ORG[0] + (x + 0.5) * cellM,
      ORG[1] + (y + 0.5) * cellM,
    ]);
    for (let it = 0; it < 2; it++) {
      const out: [number, number][] = [pts[0]];
      for (let i = 0; i < pts.length - 1; i++) {
        const [ax, ay] = pts[i];
        const [bx, by] = pts[i + 1];
        out.push([ax * 0.75 + bx * 0.25, ay * 0.75 + by * 0.25]);
        out.push([ax * 0.25 + bx * 0.75, ay * 0.25 + by * 0.75]);
      }
      out.push(pts[pts.length - 1]);
      pts = out;
    }
    let prevL: number[] | null = null;
    let prevR: number[] | null = null;
    for (let i = 0; i < pts.length; i++) {
      const [wx, wy] = pts[i];
      const z = roadY(wx, wy);
      const [fx, fy] = pts[Math.min(i + 1, pts.length - 1)];
      const [bx2, by2] = pts[Math.max(i - 1, 0)];
      let dx = fx - bx2;
      let dy = fy - by2;
      const len = Math.hypot(dx, dy) || 1;
      dx /= len;
      dy /= len;
      const L = [wx - dy * HW, z, wy + dx * HW];
      const R = [wx + dy * HW, z, wy - dx * HW];
      if (prevL && prevR) {
        rp.push(
          prevL[0], prevL[1], prevL[2],
          prevR[0], prevR[1], prevR[2],
          L[0], L[1], L[2],
          R[0], R[1], R[2],
        );
        for (let k = 0; k < 4; k++) rc.push(0.216, 0.204, 0.184);
        ri.push(base, base + 1, base + 2, base + 2, base + 1, base + 3);
        base += 4;
      }
      prevL = L;
      prevR = R;
    }
  }
  if (base > 0) {
    const rgeo = new THREE.BufferGeometry();
    rgeo.setAttribute("position", new THREE.Float32BufferAttribute(rp, 3));
    rgeo.setAttribute("color", new THREE.Float32BufferAttribute(rc, 3));
    rgeo.setIndex(ri);
    // 平面路带：法线恒朝上，避免逐段法线抖动
    const rnorm = new Float32Array(rp.length);
    for (let i = 0; i < rnorm.length; i += 3) rnorm[i + 1] = 1;
    rgeo.setAttribute("normal", new THREE.BufferAttribute(rnorm, 3));
    root.add(
      new THREE.Mesh(
        rgeo,
        new THREE.MeshLambertMaterial({ vertexColors: true, side: THREE.DoubleSide }),
      ),
    );
  }
  if (pillarMats.length) {
    const pillars = new THREE.InstancedMesh(
      new THREE.BoxGeometry(1, 1, 1),
      new THREE.MeshLambertMaterial({ color: 0x6b6257 }),
      pillarMats.length,
    );
    pillarMats.forEach((m, i) => pillars.setMatrixAt(i, m));
    root.add(pillars);
  }

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
  const loop = () => {
    raf = requestAnimationFrame(loop);
    if (!renderer || !scene || !camera || !controls) return;
    const moved = controls.update();
    // 地形碰撞：相机不得低于脚下地形 +30m（双线性采样含裁剪窗外沿）
    {
      const minY = terrain.heightWorld(camera.position.x, camera.position.z) + 30;
      if (camera.position.y < minY) {
        camera.position.y = minY;
        needsRenderFlag = true;
      }
    }
    const now = performance.now();
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
  for (const { sprite } of localizedLabels) sprite.material.map?.dispose();
  localizedLabels.length = 0;
  plotGroup = null;
  if (root) {
    root.traverse((o) => {
      const m = o as THREE.Mesh;
      if (m.geometry) m.geometry.dispose();
      const mat = m.material as THREE.Material | THREE.Material[] | undefined;
      if (Array.isArray(mat)) mat.forEach((x) => x.dispose());
      else mat?.dispose();
    });
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
  observer?.disconnect();
  controls?.dispose();
  disposeScene();
  renderer?.dispose();
  renderer?.domElement.remove();
  renderer = null;
  scene = null;
});

watch(
  () => props.data,
  (d) => {
    if (d) void build(d);
  },
);
watch(
  () => props.showPlots,
  (v) => {
    if (plotGroup) plotGroup.visible = v !== false;
  },
);
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
          if (camera && controls) {
            camera.position.set(9000, 17000, 21000);
            controls.target.set(0, 0, 0);
          }
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
