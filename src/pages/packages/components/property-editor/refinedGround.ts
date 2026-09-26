import {
  composeGroundPixels,
  copyAtlasRegion,
  nearestChannel,
  type GroundComposeInput,
  type GroundComposeResponse,
  type Pixels,
} from "./groundCompose";
import type * as ThreeNamespace from "three";

/**
 * 精细渲染的 LotMask 地面合成编排：像素提取（DOM）→ 纯核心计算（Worker
 * 优先、主线程回退，见 groundCompose.ts）→ CanvasTexture 包装。
 *
 * 主线程成本曾是 rebuild 隐藏大头（~1M 像素 ×2 张图的 JS 逐像素循环），
 * 且游离在全部遥测 span 之外——现移入 Worker，调用方用
 * `renderTelemetry.begin("texture_compose", {phase:"ground"})` 纳管。
 */

/** 反照率 + 地面法线贴图（normalMap 缺失时为 null）。 */
export interface RefinedGroundTextures {
  map: ThreeNamespace.CanvasTexture;
  normalMap: ThreeNamespace.CanvasTexture | null;
}

const groundTextureUrls = import.meta.glob<{ default: string }>(
  "../../../../assets/ground/*.png",
  { eager: true, import: "default", query: "?url" },
) as unknown as Record<string, string>;

/**
 * 图集格在地面上的一次重复的边长（米）。由消防局 0x5197EDF0（LotSize 48×48）
 * 与游戏内截图逐格比对反推：48 ÷ 9.6 = 5 次重复。**属实测拟合，非引擎常量**——
 * 引擎侧平铺次数来自地面 mesh 的 UV 跨度（引擎生成几何），生成点尚未在反编译
 * 语料定位。用户在游戏截图上数出核心铺装区约 10×15 个方格（方格 ≈ 2.4m，
 * 9.6m = 4 格，整数倍关系自洽）；casino 192×96 → 20×10 待游戏复验。
 */
const GROUND_TILE_METERS = 9.6;

const groundTiles = new Map<number, string>();
for (const [path, url] of Object.entries(groundTextureUrls)) {
  const name = path.slice(path.lastIndexOf("/") + 1, -4);
  const index = Number.parseInt(name, 10);
  if (Number.isInteger(index)) groundTiles.set(index, url);
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`ground tile failed: ${src}`));
    image.src = src;
  });
}

const tileCache = new Map<number, Promise<Pixels | null>>();
function tile(index: number): Promise<Pixels | null> {
  const url = groundTiles.get(index);
  if (!url) return Promise.resolve(null);
  let pending = tileCache.get(index);
  if (!pending) {
    pending = loadImage(url).then((image) => {
      const canvas = document.createElement("canvas");
      canvas.width = image.width;
      canvas.height = image.height;
      const context = canvas.getContext("2d");
      if (!context) return null;
      context.drawImage(image, 0, 0);
      const data = context.getImageData(0, 0, image.width, image.height);
      return { data: data.data, width: data.width, height: data.height };
    });
    tileCache.set(index, pending);
  }
  return pending.catch(() => null);
}

function imageDataPixels(image: ImageData): Pixels {
  return { data: image.data, width: image.width, height: image.height };
}

// ---------------------------------------------------------------------------
// Worker 调度（单例 + 作业表 + 断链回退主线程）
// ---------------------------------------------------------------------------

let composeWorker: Worker | null = null;
let workerBroken = false;
let nextJobId = 0;
const pendingJobs = new Map<
  number,
  { resolve: (output: GroundComposeResponse) => void; reject: () => void }
>();

function getComposeWorker(): Worker | null {
  if (composeWorker) return composeWorker;
  if (workerBroken) return null;
  try {
    composeWorker = new Worker(
      new URL("./groundComposeWorker.ts", import.meta.url),
      { type: "module" },
    );
    composeWorker.onmessage = (
      event: MessageEvent<GroundComposeResponse & { error?: string }>,
    ) => {
      const job = pendingJobs.get(event.data.id);
      if (!job) return;
      pendingJobs.delete(event.data.id);
      if (event.data.error) {
        // 单次失败（如结构化克隆不支持的字段）→ 标记断链，回退主线程。
        workerBroken = true;
        job.reject();
        return;
      }
      job.resolve(event.data);
    };
    composeWorker.onerror = () => {
      workerBroken = true;
      for (const job of pendingJobs.values()) job.reject();
      pendingJobs.clear();
    };
    return composeWorker;
  } catch {
    workerBroken = true;
    return null;
  }
}

function composeViaWorker(input: GroundComposeInput): Promise<GroundComposeResponse> {
  const worker = getComposeWorker();
  if (!worker) return Promise.reject(new Error("worker unavailable"));
  return new Promise((resolve, reject) => {
    const id = ++nextJobId;
    pendingJobs.set(id, { resolve, reject });
    // postMessage 结构化克隆即拷贝 typed array，主线程 input 不受影响。
    worker.postMessage({ id, input });
  });
}

/** 主线程回退：同一纯核心同步执行（测试环境/Worker 不可用时）。 */
function composeOnMainThread(input: GroundComposeInput): GroundComposeResponse {
  const output = composeGroundPixels(input);
  return { id: -1, ...output };
}

export async function composeRefinedGround(
  lotColors: [number, number, number, number][],
  lotColorsAuthored: boolean[],
  maskImage: TexImageSource,
  THREE: typeof ThreeNamespace,
  lotSize: [number, number] | null,
  /** 地面贴图周期 `0x0CCB7FD0`（米/格）；null 回退实测拟合常量。 */
  tilePeriod?: [number, number] | null,
  surface?: ImageData | null,
  /** 全局共享染色图集（s10）：`LotColor.A` 选格、与底图同平铺，alpha 做亮度调制。 */
  tintAtlas?: ImageData | null,
  /** 原始通道权重图；缺失时回退量化图最近色硬分配。 */
  rawMask?: ImageData | null,
  /** 全局共享法线图集（s15）：同格号、同平铺烘焙成地面 normalMap。 */
  normalAtlas?: ImageData | null,
  /** LotBorderColor1-4 的 sRGB RGB（边框带描边色；缺失 = 浅灰回退）。 */
  lotBorderColors?: [number, number, number][] | null,
  /** borderWidth1-4（边框带半宽，0..0.5）；undefined/全 0 = 无边框。 */
  lotBorderWidths?: number[] | null,
): Promise<RefinedGroundTextures | null> {
  const image = maskImage as { width?: number; height?: number };
  const width = image.width ?? 0;
  const height = image.height ?? 0;
  if (!width || !height) return null;
  // mask 像素提取（DOM；此前在 compose 内部做，现提前为 Worker 输入）。
  const maskCanvas = document.createElement("canvas");
  maskCanvas.width = width;
  maskCanvas.height = height;
  const maskContext = maskCanvas.getContext("2d");
  if (!maskContext) return null;
  maskContext.drawImage(maskImage as CanvasImageSource, 0, 0);
  const maskData = maskContext.getImageData(0, 0, width, height);
  const useSurface = Boolean(surface && surface.width >= 4 && surface.height >= 4);
  const tileW = useSurface ? Math.floor(surface!.width / 4) : 0;
  const tileH = useSurface ? Math.floor(surface!.height / 4) : 0;
  // 每通道材质源：surface 图集 tile（按 LotColor.A）优先，本地占位回退
  const channelTiles: (Pixels | null)[] = await Promise.all(
    lotColors.map((color) => {
      if (useSurface) {
        return Promise.resolve(
          copyAtlasRegion(imageDataPixels(surface!), color[3] % 16, tileW, tileH),
        );
      }
      return tile(color[3] ?? 0);
    }),
  );
  // 未覆盖区底图格 = 草地（图集 cell 8；消防局/红十字会/图书馆三楼对拍口径）
  const defaultTile = useSurface
    ? copyAtlasRegion(imageDataPixels(surface!), 8, tileW, tileH)
    : await tile(8);
  // 引擎：世界坐标除以 0x0CCB7FD0（米/格）得平铺 UV → 次数 = LotSize / 周期，
  // **非整数**（图书馆 64/10 = 6.4）。缺失时才回退旧的 9.6m 拟合常量。
  const [lotW, lotH] = lotSize ?? [0, 0];
  const periodX = tilePeriod?.[0] && tilePeriod[0] > 0 ? tilePeriod[0] : GROUND_TILE_METERS;
  const periodY = tilePeriod?.[1] && tilePeriod[1] > 0 ? tilePeriod[1] : GROUND_TILE_METERS;
  const tilesX = lotW > 0 ? Math.max(0.1, lotW / periodX) : 1;
  const tilesY = lotH > 0 ? Math.max(0.1, lotH / periodY) : 1;
  const input: GroundComposeInput = {
    mask: imageDataPixels(maskData),
    rawMask: rawMask ? imageDataPixels(rawMask) : null,
    surface: surface ? imageDataPixels(surface) : null,
    tintAtlas: tintAtlas ? imageDataPixels(tintAtlas) : null,
    normalAtlas: normalAtlas ? imageDataPixels(normalAtlas) : null,
    channelTiles,
    defaultTile,
    lotColors,
    lotColorsAuthored,
    lotBorderColors: lotBorderColors ?? null,
    lotBorderWidths: lotBorderWidths ?? null,
    tilesX,
    tilesY,
  };
  let response: GroundComposeResponse;
  try {
    response = await composeViaWorker(input);
  } catch {
    // 回退主线程（postMessage 克隆不破坏主线程 input）。
    response = composeOnMainThread(input);
  }
  const canvas = document.createElement("canvas");
  canvas.width = response.width;
  canvas.height = response.height;
  const context = canvas.getContext("2d");
  if (!context) return null;
  context.putImageData(
    new ImageData(response.albedo, response.width, response.height),
    0,
    0,
  );
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.flipY = false;
  texture.magFilter = THREE.LinearFilter;
  texture.minFilter = THREE.LinearMipmapLinearFilter;
  texture.generateMipmaps = true;
  texture.anisotropy = 8;
  let normalMap: ThreeNamespace.CanvasTexture | null = null;
  if (response.normal) {
    const normalCanvas = document.createElement("canvas");
    normalCanvas.width = response.width;
    normalCanvas.height = response.height;
    const normalContext = normalCanvas.getContext("2d");
    if (normalContext) {
      normalContext.putImageData(
        new ImageData(response.normal, response.width, response.height),
        0,
        0,
      );
      normalMap = new THREE.CanvasTexture(normalCanvas);
      // 法线为线性数据：保持 NoColorSpace（不标 sRGB），否则 three 会做
      // sRGB→线性解码把方向压偏。
      normalMap.flipY = false;
      normalMap.magFilter = THREE.LinearFilter;
      normalMap.minFilter = THREE.LinearMipmapLinearFilter;
      normalMap.generateMipmaps = true;
      normalMap.anisotropy = 8;
    }
  }
  return { map: texture, normalMap };
}

/**
 * 自动锚定：在量化 mask 图上找"主足迹色区"（最大非满铺色区，退化时
 * 取全部着色像素质心），返回以地面矩形中心为原点的局部坐标偏移。
 * 用于把足迹色区对齐到建筑 bbox 中心——绕开 placement 正/逆约定，
 * 直接解决"色区在 raster 角落时建筑居中导致的偏移"（用户实测）。
 */
export function maskAnchorOffset(
  maskImage: TexImageSource,
  lotColors: [number, number, number, number][],
  lotSize: [number, number],
): { x: number; y: number } | null {
  const image = maskImage as { width?: number; height?: number };
  const width = image.width ?? 0;
  const height = image.height ?? 0;
  if (!width || !height) return null;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) return null;
  context.drawImage(maskImage as CanvasImageSource, 0, 0);
  const data = context.getImageData(0, 0, width, height).data;
  const bounds = Array.from({ length: 4 }, () => ({
    x0: Number.POSITIVE_INFINITY,
    y0: Number.POSITIVE_INFINITY,
    x1: Number.NEGATIVE_INFINITY,
    y1: Number.NEGATIVE_INFINITY,
    count: 0,
  }));
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const at = (y * width + x) * 4;
      if (data[at + 3] < 16) continue;
      const channel = nearestChannel(data[at], data[at + 1], data[at + 2], lotColors);
      const slot = bounds[channel];
      slot.count += 1;
      slot.x0 = Math.min(slot.x0, x);
      slot.y0 = Math.min(slot.y0, y);
      slot.x1 = Math.max(slot.x1, x);
      slot.y1 = Math.max(slot.y1, y);
    }
  }
  const candidates = bounds
    .map((slot, index) => ({ slot, index }))
    .filter(({ slot }) => slot.count > 0);
  const nonFullBleed = candidates.filter(
    ({ slot }) =>
      (slot.x1 - slot.x0 + 1) / width <= 0.95 ||
      (slot.y1 - slot.y0 + 1) / height <= 0.95,
  );
  let cx: number;
  let cy: number;
  if (nonFullBleed.length) {
    const dominant = nonFullBleed.reduce((a, b) => (b.slot.count > a.slot.count ? b : a));
    cx = (dominant.slot.x0 + dominant.slot.x1) / 2;
    cy = (dominant.slot.y0 + dominant.slot.y1) / 2;
  } else {
    let sumX = 0;
    let sumY = 0;
    let count = 0;
    for (let y = 0; y < height; y += 1) {
      for (let x = 0; x < width; x += 1) {
        const at = (y * width + x) * 4;
        if (data[at + 3] < 16) continue;
        sumX += x;
        sumY += y;
        count += 1;
      }
    }
    if (!count) return null;
    cx = sumX / count;
    cy = sumY / count;
  }
  return {
    x: (cx / width - 0.5) * lotSize[0],
    y: (cy / height - 0.5) * lotSize[1],
  };
}
