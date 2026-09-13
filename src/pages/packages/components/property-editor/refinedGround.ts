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
 * 同时烘焙同口径的 **法线贴图**（s15 共享图集）：格号、平铺与反照率逐像素一致
 * （仅未覆盖区改用底图格），故两者像素对齐、不存在相位错位。法线图集是标准
 * 切线空间（平坦 = RGB(128,128,255)），线性空间（不得标 sRGB）。
 *
 * 画布即引擎空间：mask 列序与模型 X 同向（lot_mask_alignment 裁定 identity）、
 * 行序翻转由后端统一完成，故 tile 采样 u/v 均直取（无镜像）。探针出图里的
 * `u = 1-x` 是其 PNG 坐标系专属，勿搬回画布。
 *
 * 着色规则：LotColor 属性缺失（authored=false）时回退色（黑/红/绿/蓝）
 * 只是编辑器可视化，不参与着色——直接铺贴图原色（用户实测 2026-09-10）。
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
  /** 地面贴图周期 `0x0CCB7FD0`（米/格）；null 回退实测拟合常量。 */
  tilePeriod?: [number, number] | null,
  surface?: ImageData | null,
  /** 全局共享染色图集（s10）：`LotColor.A` 选格、与底图同平铺，alpha 做亮度调制。 */
  tintAtlas?: ImageData | null,
  /** 原始通道权重图；缺失时回退量化图最近色硬分配。 */
  rawMask?: ImageData | null,
  /** 全局共享法线图集（s15）：同格号、同平铺烘焙成地面 normalMap。 */
  normalAtlas?: ImageData | null,
): Promise<RefinedGroundTextures | null> {
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
  // 法线图集（s15）：4×4 格、每格 >0 才有意义。
  const normalCellW = normalAtlas ? Math.floor(normalAtlas.width / 4) : 0;
  const normalCellH = normalAtlas ? Math.floor(normalAtlas.height / 4) : 0;
  const useNormal = normalCellW >= 1 && normalCellH >= 1;
  // 法线图另开一张同尺寸画布：与反照率共用 (u,v)→像素映射，天然像素对齐。
  const normalCanvas = document.createElement("canvas");
  normalCanvas.width = outW;
  normalCanvas.height = outH;
  const normalContext = useNormal ? normalCanvas.getContext("2d") : null;
  const normalComposed = normalContext
    ? normalContext.createImageData(outW, outH)
    : null;
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

  /** 染色图集（s10）按格复制（4×4）；格号同 LotColor.A，与底图同平铺。 */
  const tintCellW = tintAtlas ? Math.floor(tintAtlas.width / 4) : 0;
  const tintCellH = tintAtlas ? Math.floor(tintAtlas.height / 4) : 0;
  function copyRegionFromTint(index: number): ImageData | null {
    if (!tintAtlas || tintCellW < 1 || tintCellH < 1) return null;
    const out = new ImageData(tintCellW, tintCellH);
    const originX = (index % 4) * tintCellW;
    const originY = Math.floor(index / 4) * tintCellH;
    for (let y = 0; y < tintCellH; y += 1) {
      const srcRow = ((originY + y) * tintAtlas.width + originX) * 4;
      out.data.set(
        tintAtlas.data.subarray(srcRow, srcRow + tintCellW * 4),
        y * tintCellW * 4,
      );
    }
    return out;
  }
  const tintCells = tintAtlas
    ? lotColors.map((color) => copyRegionFromTint(color[3] % 16))
    : null;

  /** 法线图集（s15）按格复制（4×4）；格号同 `LotColor.A`，与底图同平铺。 */
  function copyRegionFromNormal(index: number): ImageData | null {
    if (!normalAtlas || !useNormal) return null;
    const out = new ImageData(normalCellW, normalCellH);
    const originX = (index % 4) * normalCellW;
    const originY = Math.floor(index / 4) * normalCellH;
    for (let y = 0; y < normalCellH; y += 1) {
      const srcRow = ((originY + y) * normalAtlas.width + originX) * 4;
      out.data.set(
        normalAtlas.data.subarray(srcRow, srcRow + normalCellW * 4),
        y * normalCellW * 4,
      );
    }
    return out;
  }
  const normalCells = useNormal
    ? lotColors.map((color) => copyRegionFromNormal(color[3] % 16))
    : null;
  const normalDefault = useNormal ? copyRegionFromNormal(8) : null;

  /** 染色图集 alpha（0..1），平铺口径同底图。 */
  function sampleTiledAlpha(
    source: ImageData,
    u: number,
    v: number,
    tx: number,
    ty: number,
  ): number {
    const fu = u * tx;
    const fv = v * ty;
    const px = Math.min(source.width - 1, Math.floor((fu - Math.floor(fu)) * source.width));
    const py = Math.min(source.height - 1, Math.floor((fv - Math.floor(fv)) * source.height));
    return source.data[(py * source.width + px) * 4 + 3] / 255;
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

  // 引擎：世界坐标除以 0x0CCB7FD0（米/格）得平铺 UV → 次数 = LotSize / 周期，
  // **非整数**（图书馆 64/10 = 6.4）。缺失时才回退旧的 9.6m 拟合常量。
  const [lotW, lotH] = lotSize ?? [0, 0];
  const periodX = tilePeriod?.[0] && tilePeriod[0] > 0 ? tilePeriod[0] : GROUND_TILE_METERS;
  const periodY = tilePeriod?.[1] && tilePeriod[1] > 0 ? tilePeriod[1] : GROUND_TILE_METERS;
  const tilesX = lotW > 0 ? Math.max(0.1, lotW / periodX) : 1;
  const tilesY = lotH > 0 ? Math.max(0.1, lotH / periodY) : 1;

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
        let r8 = (tr * tint[0]) / 255;
        let g8 = (tg * tint[1]) / 255;
        let b8 = (tb * tint[2]) / 255;
        // s10 染色图集调制：车辙/铺装的明暗细节在此（覆盖区 overlayMask=1）。
        if (channel >= 0 && tintCells?.[channel]) {
          const mul = Math.min(2, sampleTiledAlpha(tintCells[channel]!, u, v, tilesX, tilesY) * 2);
          r8 = Math.min(255, r8 * mul);
          g8 = Math.min(255, g8 * mul);
          b8 = Math.min(255, b8 * mul);
        }
        composed.data[at] = r8;
        composed.data[at + 1] = g8;
        composed.data[at + 2] = b8;
      } else {
        composed.data[at] = 58;
        composed.data[at + 1] = 62;
        composed.data[at + 2] = 54;
      }
      // 法线：同格（覆盖区 = 胜出通道 LotColor.A，未覆盖区 = 底图格）、同平铺
      // 频率——与反照率逐像素对齐。无格可采时退化为平坦法线 (128,128,255)。
      if (normalComposed) {
        const normalSource =
          channel >= 0 ? normalCells?.[channel] ?? null : normalDefault;
        if (normalSource) {
          const [nr, ng, nb] = sampleTiled(normalSource, u, v, tilesX, tilesY);
          normalComposed.data[at] = nr;
          normalComposed.data[at + 1] = ng;
          normalComposed.data[at + 2] = nb;
        } else {
          normalComposed.data[at] = 128;
          normalComposed.data[at + 1] = 128;
          normalComposed.data[at + 2] = 255;
        }
        normalComposed.data[at + 3] = 255;
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
  let normalMap: ThreeNamespace.CanvasTexture | null = null;
  if (normalContext && normalComposed) {
    normalContext.putImageData(normalComposed, 0, 0);
    normalMap = new THREE.CanvasTexture(normalCanvas);
    // 法线为线性数据：保持 NoColorSpace（不标 sRGB），否则 three 会做
    // sRGB→线性解码把方向压偏。
    normalMap.flipY = false;
    normalMap.magFilter = THREE.LinearFilter;
    normalMap.minFilter = THREE.LinearMipmapLinearFilter;
    normalMap.generateMipmaps = true;
    normalMap.anisotropy = 8;
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
