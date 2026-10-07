import {
  composeGroundPixels,
  copyAtlasRegion,
  groundOutputSize,
  nearestChannel,
  type GroundComposeInput,
  type GroundComposeResponse,
  type Pixels,
} from "./groundCompose";
import type * as ThreeNamespace from "three";

/**
 * 精细渲染的 LotMask 地面合成编排：像素提取（DOM）→ 纯核心计算（Worker
 * 优先、主线程回退，见 groundCompose.ts）→ CanvasTexture 包装。引擎口径
 * （generic_lot）：覆盖区平色 × 图案坡度明暗（烘焙进反照率），未覆盖区底图格。
 *
 * 主线程成本曾是 rebuild 隐藏大头（~1M 像素 ×2 张图的 JS 逐像素循环），
 * 且游离在全部遥测 span 之外——现移入 Worker，调用方用
 * `renderTelemetry.begin("texture_compose", {phase:"ground"})` 纳管。
 */

const groundTextureUrls = import.meta.glob<{ default: string }>(
  "../../../../assets/ground/*.png",
  { eager: true, import: "default", query: "?url" },
) as unknown as Record<string, string>;

/**
 * 图集格在地面上的一次重复的边长（米）。仅作 `0x0CCB7FD0` 与 LotSize 双缺
 * 时的最后回退——引擎真值 = LotSize / 0x0CCB7FD0（平铺次数 = LotSize ÷ 周期，
 * 非整数）。消防局 48 ÷ 9.6 = 5 为 2026-09 早期实测拟合，非引擎常量。
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

/** Start the module worker while source textures are still decoding. */
export function prepareGroundComposer(): void {
  getComposeWorker();
}

function composeViaWorker(
  input: GroundComposeInput,
  transfers: ArrayBuffer[],
): Promise<GroundComposeResponse> {
  const worker = getComposeWorker();
  if (!worker) return Promise.reject(new Error("worker unavailable"));
  return new Promise((resolve, reject) => {
    const id = ++nextJobId;
    pendingJobs.set(id, { resolve, reject });
    // transferable 零拷贝：postMessage 默认结构化克隆整份像素输入
    // （2048² 级 = 数十 MB 主线程拷贝）。仅转移本次调用的新鲜产物；
    // 共享缓存的 buffer（tile() 回退/normalAtlas）必须走克隆路径。
    worker.postMessage({ id, input }, transfers);
  });
}

/** 主线程回退：同一纯核心同步执行（测试环境/Worker 不可用时）。 */
function composeOnMainThread(input: GroundComposeInput): GroundComposeResponse {
  const output = composeGroundPixels(input);
  return { id: -1, ...output };
}

export async function composeRefinedGround(options: {
  lotColors: [number, number, number, number][];
  maskImage: TexImageSource;
  THREE: typeof ThreeNamespace;
  /** LotSize（米）；null 回退 9.6m 拟合常量。 */
  lotSize?: [number, number] | null;
  /** 地面贴图周期 `0x0CCB7FD0`（米/格）；null 回退实测拟合常量。 */
  tilePeriod?: [number, number] | null;
  /** "Lot Textures" 地表共享纹理图集像素（底图格来源）。 */
  surface?: ImageData | null;
  /** 底图格索引（后端三级来源解析结果）。 */
  baseTileIndex: number;
  /** 全局共享法线图集像素（s15；图案坡度明暗烘焙来源）。 */
  normalAtlas?: ImageData | null;
  /** 原始通道权重图；缺失时回退量化图最近色硬分配。 */
  rawMask?: ImageData | null;
  /** LotBorderColor1-4 的 sRGB RGB。 */
  lotBorderColors?: [number, number, number][] | null;
  /** 边框带图案索引（LotBorderColor.A）。 */
  lotBorderPatternIndices?: number[] | null;
  /** borderWidth1-4（边框带半宽，0..0.5）；undefined/全 0 = 无边框。 */
  lotBorderWidths?: number[] | null;
}): Promise<ThreeNamespace.CanvasTexture | null> {
  const {
    THREE,
    maskImage,
    lotSize,
    tilePeriod,
    surface,
    baseTileIndex,
    normalAtlas,
    rawMask,
    lotBorderColors,
    lotBorderPatternIndices,
    lotBorderWidths,
  } = options;
  const lotColors = options.lotColors;
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
  // 未覆盖区底图格：数据驱动索引（后端三级来源），surface 图集切格，
  // 缺失回退本地占位 tile。
  const baseTile = useSurface
    ? copyAtlasRegion(imageDataPixels(surface!), baseTileIndex % 16, tileW, tileH)
    : await tile(baseTileIndex % 16);
  // 引擎：世界坐标除以 0x0CCB7FD0（米/格）得平铺 UV → 次数 = LotSize / 周期，
  // **非整数**（图书馆 64/10 = 6.4）。缺失时才回退旧的 9.6m 拟合常量。
  const [lotW, lotH] = lotSize ?? [0, 0];
  const periodX = tilePeriod?.[0] && tilePeriod[0] > 0 ? tilePeriod[0] : GROUND_TILE_METERS;
  const periodY = tilePeriod?.[1] && tilePeriod[1] > 0 ? tilePeriod[1] : GROUND_TILE_METERS;
  const tilesX = lotW > 0 ? Math.max(0.1, lotW / periodX) : 1;
  const tilesY = lotH > 0 ? Math.max(0.1, lotH / periodY) : 1;
  // 输出尺寸按图案格原生密度预算（= 探针 hires 的 32px/m 口径）：每个图案
  // 重复恰好一格分辨率，烘焙 1:1 最清晰。仅 LotSize 已知时启用（否则 tiles
  // 是回退值，密度无意义）；核心侧还有"不低于 mask×4"守卫与 2048 上限。
  const patternCellPx =
    normalAtlas && normalAtlas.width >= 4 && normalAtlas.height >= 4
      ? Math.floor(normalAtlas.width / 4)
      : 0;
  const outSize =
    lotSize && patternCellPx >= 4
      ? groundOutputSize(width, height, tilesX, tilesY, patternCellPx)
      : null;
  const input: GroundComposeInput = {
    mask: imageDataPixels(maskData),
    rawMask: rawMask ? imageDataPixels(rawMask) : null,
    baseTile,
    lotColors,
    lotBorderColors: lotBorderColors ?? null,
    lotBorderPatternIndices: lotBorderPatternIndices ?? null,
    lotBorderWidths: lotBorderWidths ?? null,
    normalAtlas: normalAtlas ? imageDataPixels(normalAtlas) : null,
    tilesX,
    tilesY,
    outSize,
  };
  // 可转移 buffer：mask 像素恒为本次新提取；baseTile 在 surface 图集切格
  // 分支是新拷贝（copyAtlasRegion），本地 tile() 回退分支是共享缓存（不可
  // 转移，转移会把缓存 buffer detach）。
  const transfers: ArrayBuffer[] = [input.mask.data.buffer];
  if (useSurface && baseTile) transfers.push(baseTile.data.buffer);
  let response: GroundComposeResponse;
  try {
    response = await composeViaWorker(input, transfers);
  } catch {
    // 回退主线程。转移过的 buffer 已 detach（mask 画布内容仍在，重新
    // getImageData；baseTile 按来源重取）。
    const remask = maskContext.getImageData(0, 0, width, height);
    const retryInput: GroundComposeInput = {
      ...input,
      mask: imageDataPixels(remask),
      baseTile: useSurface
        ? copyAtlasRegion(imageDataPixels(surface!), baseTileIndex % 16, tileW, tileH)
        : await tile(baseTileIndex % 16),
    };
    response = composeOnMainThread(retryInput);
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
  return texture;
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
