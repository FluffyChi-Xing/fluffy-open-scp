import type * as ThreeNamespace from "three";

/**
 * 精细渲染的 LotMask 地面合成（引擎语义，= lot_compose 探针 compose_tiles
 * 同口径）：
 *  1. 选区：mask 四通道权重 >0.5 硬阈值，按引擎优先级链 A>B>G>R（w→z→y→x）
 *     选出唯一通道；
 *  2. 材质：胜出通道铺 tile_{LotColor.A}（16 格共享图集），tile 按
 *     frac(uv × N) 平铺（N = LotSize / GROUND_TILE_METERS，逐轴）；
 *  3. 着色：胜出通道 LotColor.RGB（后端已 sRGB 字节）乘 tile 原色；
 *  4. 未覆盖区：铺底图格（图集 cell 8 草地；三楼对拍口径），不透明。
 *
 * 画布即引擎空间：mask 列序与模型 X 同向（lot_mask_alignment 裁定 identity）、
 * 行序翻转由后端统一完成，故 tile 采样 u/v 均直取（无镜像）。探针出图里的
 * `u = 1-x` 是其 PNG 坐标系专属，勿搬回画布。
 *
 * 着色规则：LotColor 属性缺失（authored=false）时回退色（黑/红/绿/蓝）
 * 只是编辑器可视化，不参与着色——直接铺贴图原色（用户实测 2026-09-10）。
 */

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

const tileCache = new Map<number, Promise<ImageData | null>>();
function tile(index: number): Promise<ImageData | null> {
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
      return context.getImageData(0, 0, image.width, image.height);
    });
    tileCache.set(index, pending);
  }
  return pending.catch(() => null);
}

function nearestChannel(
  r: number,
  g: number,
  b: number,
  colors: [number, number, number, number][],
): number {
  let best = -1;
  let bestDistance = Number.POSITIVE_INFINITY;
  for (let i = 0; i < colors.length; i += 1) {
    const [cr, cg, cb] = colors[i];
    const distance = (r - cr) ** 2 + (g - cg) ** 2 + (b - cb) ** 2;
    if (distance < bestDistance) {
      bestDistance = distance;
      best = i;
    }
  }
  return best;
}

export async function composeRefinedGround(
  lotColors: [number, number, number, number][],
  lotColorsAuthored: boolean[],
  maskImage: TexImageSource,
  THREE: typeof ThreeNamespace,
  lotSize: [number, number] | null,
  surface?: ImageData | null,
  /** 原始通道权重图；缺失时回退量化图最近色硬分配。 */
  rawMask?: ImageData | null,
): Promise<ThreeNamespace.CanvasTexture | null> {
  const image = maskImage as { width?: number; height?: number };
  const width = image.width ?? 0;
  const height = image.height ?? 0;
  if (!width || !height) return null;
  // 4× 超采样输出：mask 权重双线性插值 + tile 原生分辨率采样，
  // 消除 128px 权重图直贴 64m 地面的模糊（对拍 2026-09-12）。
  const scale = Math.min(4, Math.max(1, Math.floor(1024 / Math.max(width, height))));
  const outW = width * scale;
  const outH = height * scale;
  const canvas = document.createElement("canvas");
  canvas.width = outW;
  canvas.height = outH;
  const context = canvas.getContext("2d");
  if (!context) return null;
  context.drawImage(maskImage as CanvasImageSource, 0, 0);
  const mask = context.getImageData(0, 0, width, height);
  const composed = context.createImageData(outW, outH);
  const useSurface = Boolean(surface && surface.width >= 4 && surface.height >= 4);
  const tileW = useSurface ? Math.floor(surface!.width / 4) : 0;
  const tileH = useSurface ? Math.floor(surface!.height / 4) : 0;

    /** 画布 UV → raw mask 通道权重（最近邻）。必须与默认反照率同口径：
     *  双线性会在过渡带里让高优先级通道压过邻区（图书馆绿带外扩吃掉
     *  米色地坪，2026-09-13 对拍），区域划分以 mask 像素为准。 */
    function sampleWeights(u: number, v: number): [number, number, number, number] {
      const raw = rawMask;
      if (!raw) return [0, 0, 0, 0];
      const mx = Math.min(raw.width - 1, Math.floor(u * raw.width));
      const my = Math.min(raw.height - 1, Math.floor(v * raw.height));
      const at = (my * raw.width + mx) * 4;
      return [
        raw.data[at] / 255,
        raw.data[at + 1] / 255,
        raw.data[at + 2] / 255,
        raw.data[at + 3] / 255,
      ];
    }

  /** 从图集 ImageData 复制第 index 格（4×4）。 */
  function copyRegionFromSurface(index: number): ImageData | null {
    if (!surface) return null;
    const out = new ImageData(tileW, tileH);
    const originX = (index % 4) * tileW;
    const originY = Math.floor(index / 4) * tileH;
    for (let y = 0; y < tileH; y += 1) {
      const srcRow = ((originY + y) * surface.width + originX) * 4;
      out.data.set(surface.data.subarray(srcRow, srcRow + tileW * 4), y * tileW * 4);
    }
    return out;
  }

  // 每通道材质源：surface 图集 tile（按 LotColor.A）优先，本地占位回退
  const channelTiles = await Promise.all(
    lotColors.map((color) => {
      if (useSurface) {
        return Promise.resolve(copyRegionFromSurface(color[3] % 16));
      }
      return tile(color[3] ?? 0);
    }),
  );
  // 未覆盖区底图格 = 草地（图集 cell 8；消防局/红十字会/图书馆三楼对拍口径）
  const defaultTile = useSurface ? copyRegionFromSurface(8) : await tile(8);

  /**
   * 平铺采样：引擎 frac(baseUV)——UV 跨 0..N 该格重复 N 次。次数由
   * LotSize / GROUND_TILE_METERS 逐轴取整（fire 48m → 5×5）。
   */
  function sampleTiled(
    source: ImageData,
    u: number,
    v: number,
    tilesX: number,
    tilesY: number,
  ): [number, number, number] {
    const fu = u * tilesX;
    const fv = v * tilesY;
    const px = Math.min(
      source.width - 1,
      Math.floor((fu - Math.floor(fu)) * source.width),
    );
    const py = Math.min(
      source.height - 1,
      Math.floor((fv - Math.floor(fv)) * source.height),
    );
    const offset = (py * source.width + px) * 4;
    return [source.data[offset], source.data[offset + 1], source.data[offset + 2]];
  }

  const [lotW, lotH] = lotSize ?? [0, 0];
  const tilesX = lotW > 0 ? Math.max(1, Math.round(lotW / GROUND_TILE_METERS)) : 1;
  const tilesY = lotH > 0 ? Math.max(1, Math.round(lotH / GROUND_TILE_METERS)) : 1;

  for (let y = 0; y < outH; y += 1) {
    for (let x = 0; x < outW; x += 1) {
      const at = (y * outW + x) * 4;
      const u = x / outW;
      const v = y / outH;
      // 引擎优先级链：w→z→y→x = A > B > G > R；>0.5 硬阈值选区。
      let channel = -1;
      if (rawMask) {
        const weights = sampleWeights(u, v);
        for (const c of [3, 2, 1, 0]) {
          if (weights[c] > 0.5) {
            channel = c;
            break;
          }
        }
      } else {
        // v1 回退：量化图（RGB = 通道色字节）最近色硬分配，alpha 即覆盖。
        const mx = Math.min(width - 1, Math.floor(u * width));
        const my = Math.min(height - 1, Math.floor(v * height));
        const mat = (my * width + mx) * 4;
        if (mask.data[mat + 3] >= 16) {
          channel = nearestChannel(
            mask.data[mat],
            mask.data[mat + 1],
            mask.data[mat + 2],
            lotColors,
          );
        }
      }
      const source = channel >= 0 ? channelTiles[channel] : defaultTile;
      if (source) {
        const [tr, tg, tb] = sampleTiled(source, u, v, tilesX, tilesY);
        const tint =
          channel >= 0 && lotColorsAuthored[channel]
            ? [lotColors[channel][0], lotColors[channel][1], lotColors[channel][2]]
            : [255, 255, 255];
        composed.data[at] = (tr * tint[0]) / 255;
        composed.data[at + 1] = (tg * tint[1]) / 255;
        composed.data[at + 2] = (tb * tint[2]) / 255;
      } else {
        composed.data[at] = 58;
        composed.data[at + 1] = 62;
        composed.data[at + 2] = 54;
      }
      composed.data[at + 3] = 255;
    }
  }
  context.putImageData(composed, 0, 0);
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
