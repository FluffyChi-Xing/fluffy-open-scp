/**
 * 精细地面合成的**纯像素核心**（无 DOM 依赖）——主线程与 Worker 共用。
 *
 * 引擎语义（`generic_lot` 像素着色器逐字直译，= lot_composite.rs 校准口径，
 * 见 docs/blog/raster-lot-rendering.md §3/§5/§7）：
 *  1. 选区：raw mask 四通道权重 > 0.5−borderWidth 硬阈值 one-hot，按引擎
 *     优先级瀑布 A边框 > A主色 > B边框 > B主色 > G边框 > G主色 > R边框 >
 *     R主色（w→z→y→x）选出唯一胜者；边框带 = 权重 ∈ (0.5−bw, 0.5+bw]；
 *  2. 反照率：胜者输出**平色**（主区 LotColor.RGB / 边框带
 *     LotBorderColor.RGB，后端已 linear→sRGB）——引擎覆盖区不采样漫反射
 *     （tile×tint 双重变暗已证伪，博客 §7.3）；
 *  3. 图案质感：胜者格号（主区 = LotColor.A / 边框带 = LotBorderColor.A）
 *     选法线图集 0x60E7805D 的 4×4 格，按 (u−0.5)·tiles 相位平铺（单 lot
 *     视图的引擎 uv1 世界锚定等价形式），**坡度明暗直接烘焙进反照率**
 *     （线性空间相乘后回 sRGB）。烘焙而非 normalMap 实时光照的裁定：
 *     引擎 lotCalcLighting 是强风格化项（切线空间线性项 ×1.4，明暗差可达
 *     ±45%，lot_composite shade_of 同款对拍校准），而真实法线在平射阳光下
 *     的 N·L 响应是二阶小量——PE 曾用 normalMap+Phong 结果图案不可见
 *     （2026-09-29 用户对拍：精细与默认几乎无差异）；
 *  4. 未覆盖区：底图格（Lot Textures 图集第 baseTile 格，数据驱动三级来源）
 *     **整格拉伸**铺满地块，图集 U 轴与 mask 列序相反（§5b，采样 u = 1−u），
 *     无图案光照（引擎 overlayMask=0 处）。
 *
 * 画布即引擎空间：后端已做行序翻转（row 0 = 北/+Y），mask 列 0 = 西（−X）
 * 直采；4× 超采样 + 双线性权重 = GPU 口径（阈值在插值之后）。
 */

export interface Pixels {
  data: Uint8ClampedArray<ArrayBuffer>;
  width: number;
  height: number;
}

export interface GroundComposeInput {
  /** 量化 mask 图（RGB = 通道色字节，alpha = 覆盖）——rawMask 缺失时的回退源。 */
  mask: Pixels;
  /** 原始通道权重图；缺失时回退量化图最近色硬分配。 */
  rawMask: Pixels | null;
  /** 底图格像素（Lot Textures 图集第 baseTile 格已切出）；null = 白。 */
  baseTile: Pixels | null;
  lotColors: [number, number, number, number][];
  /** LotBorderColor1-4 的 sRGB RGB；null = 无边框带。 */
  lotBorderColors: [number, number, number][] | null;
  /** 边框带图案索引（LotBorderColor.A，0-15）；null = 全 0。 */
  lotBorderPatternIndices: number[] | null;
  /** borderWidth1-4（边框带半宽，0..0.5）；null/全 0 = 无边框。 */
  lotBorderWidths: number[] | null;
  /** 全局共享法线图集（4×4 格，0x60E7805D）；null = 无图案光照（纯平色）。 */
  normalAtlas: Pixels | null;
  /** 图案平铺次数（逐轴）= LotSize / 0x0CCB7FD0（非整数）。 */
  tilesX: number;
  tilesY: number;
  /**
   * 输出尺寸覆盖（编排方按图案原生密度预算的结果，groundOutputSize 同款）；
   * null = 核心内回退 mask×4。
   */
  outSize?: { width: number; height: number } | null;
}

export interface GroundComposeOutput {
  albedo: Uint8ClampedArray<ArrayBuffer>;
  width: number;
  height: number;
}

/** sRGB 字节 → 线性 0..1（lot_composite 同公式）。 */
function srgbToLinear(byte: number): number {
  const srgb = byte / 255;
  return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
}

/** 线性 0..1 → sRGB 字节。 */
function linearToSrgbByte(linear: number): number {
  const clamped = Math.min(1, Math.max(0, Number.isFinite(linear) ? linear : 0));
  const srgb =
    clamped <= 0.0031308 ? 12.92 * clamped : 1.055 * clamped ** (1 / 2.4) - 0.055;
  return Math.round(srgb * 255);
}

/** v1 回退路径的最近色硬分配（量化 mask RGB → 通道下标）。 */
export function nearestChannel(
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

/** 从 4×4 图集切第 index 格为独立 Pixels（纯数组切片，Worker 可用）。 */
export function copyAtlasRegion(
  atlas: Pixels,
  index: number,
  cellW: number,
  cellH: number,
): Pixels | null {
  if (cellW < 1 || cellH < 1) return null;
  const out = new Uint8ClampedArray(cellW * cellH * 4);
  const originX = (index % 4) * cellW;
  const originY = Math.floor(index / 4) * cellH;
  for (let y = 0; y < cellH; y += 1) {
    const srcRow = ((originY + y) * atlas.width + originX) * 4;
    out.set(atlas.data.subarray(srcRow, srcRow + cellW * 4), y * cellW * 4);
  }
  return { data: out, width: cellW, height: cellH };
}

/** fract 到 [0,1)（JS % 对负数返回负值，不能直接用）。 */
function frac(value: number): number {
  return value - Math.floor(value);
}

/**
 * 底图格整格拉伸采样（最近邻）：u 经 1−u 镜像（图集 U 轴与 mask 列序相反，
 * §5b）。最近邻是探针 hires 同款（用户对拍裁定：颗粒感 > 双线性的模糊）——
 * 底图整格拉伸的放大倍率很大（216m lot ≈ 16×），双线性会抹掉格内纹理。
 */
function sampleBaseCell(
  source: Pixels,
  u: number,
  v: number,
): [number, number, number] {
  const px = Math.min(
    source.width - 1,
    Math.floor((1 - u) * source.width),
  );
  const py = Math.min(source.height - 1, Math.floor(v * source.height));
  const offset = (py * source.width + px) * 4;
  return [source.data[offset], source.data[offset + 1], source.data[offset + 2]];
}

/**
 * 图案格平铺采样（双线性，wrap）：引擎 uv1 相位 (u−0.5)·tiles（单 lot 等价
 * 形式）。平铺重复之间无缝 wrap；输出密度≈格原生密度（1:1）时退化为最近邻
 * ——即探针口径；密度不足（上限裁剪）时双线性软化而非最近邻丢线。
 */
function samplePatternBilinear(
  source: Pixels,
  u: number,
  v: number,
  tilesX: number,
  tilesY: number,
): [number, number, number] {
  const fx = frac((u - 0.5) * tilesX) * source.width - 0.5;
  const fy = frac((v - 0.5) * tilesY) * source.height - 0.5;
  const x0 = Math.floor(fx);
  const y0 = Math.floor(fy);
  const tx = fx - x0;
  const ty = fy - y0;
  const wrap = (value: number): number =>
    value < 0
      ? value + source.width
      : value >= source.width
        ? value - source.width
        : value;
  const wrapY = (value: number): number =>
    value < 0
      ? value + source.height
      : value >= source.height
        ? value - source.height
        : value;
  const x1 = wrap(x0 + 1);
  const y1 = wrapY(y0 + 1);
  const at = (x: number, y: number): [number, number, number] => {
    const offset = (y * source.width + x) * 4;
    return [source.data[offset], source.data[offset + 1], source.data[offset + 2]];
  };
  const topLeft = at(wrap(x0), y0);
  const topRight = at(x1, y0);
  const bottomLeft = at(wrap(x0), y1);
  const bottomRight = at(x1, y1);
  const out: [number, number, number] = [0, 0, 0];
  for (let c = 0; c < 3; c += 1) {
    const top = topLeft[c] + (topRight[c] - topLeft[c]) * tx;
    const bottom = bottomLeft[c] + (bottomRight[c] - bottomLeft[c]) * tx;
    out[c] = top + (bottom - top) * ty;
  }
  return out;
}

/** 输出画布上限（等比缩，保长宽比）= 探针 hires 同款上限（lot_composite MAX_SIDE）。 */
const OUTPUT_MAX_SIDE = 4096;

/**
 * 输出尺寸：优先按**图案格原生密度**（cellPx/tilePeriod px 每米，典型
 * 256/8 = 32px/m = 探针 hires 口径）——每个图案重复恰好一格分辨率，烘焙
 * 1:1 采样最清晰；LotSize 未知或密度低于 mask×4 时回退 mask×4（上限 1024，
 * 既有口径）。超上限等比缩。
 */
export function groundOutputSize(
  maskWidth: number,
  maskHeight: number,
  tilesX: number,
  tilesY: number,
  patternCellPx: number,
): { width: number; height: number } {
  const fallbackScale = Math.min(
    4,
    Math.max(1, Math.floor(1024 / Math.max(maskWidth, maskHeight))),
  );
  let outW = maskWidth * fallbackScale;
  let outH = maskHeight * fallbackScale;
  if (patternCellPx >= 4 && tilesX > 0 && tilesY > 0) {
    const idealW = Math.round(tilesX * patternCellPx);
    const idealH = Math.round(tilesY * patternCellPx);
    // 不低于 mask×4 的超采样水平才启用（否则反而降低 mask 边缘质量）。
    if (Math.min(idealW, idealH) >= Math.max(maskWidth, maskHeight) * 2) {
      outW = idealW;
      outH = idealH;
    }
  }
  const shrink = Math.min(1, OUTPUT_MAX_SIDE / Math.max(outW, outH));
  return {
    width: Math.max(1, Math.round(outW * shrink)),
    height: Math.max(1, Math.round(outH * shrink)),
  };
}

/**
 * 合成主循环（纯函数）。输出密度 = 图案格原生（groundOutputSize）；mask
 * 权重双线性插值（阈值在插值后逐像素施加 = GPU 口径）+ 平色×图案明暗/
 * 底图直出。
 */
export function composeGroundPixels(input: GroundComposeInput): GroundComposeOutput {
  const { mask, rawMask } = input;
  const width = mask.width;
  const height = mask.height;
  const useNormal = input.normalAtlas !== null &&
    Math.floor(input.normalAtlas.width / 4) >= 1 &&
    Math.floor(input.normalAtlas.height / 4) >= 1;
  const normalCellW = useNormal ? Math.floor(input.normalAtlas!.width / 4) : 0;
  const normalCellH = useNormal ? Math.floor(input.normalAtlas!.height / 4) : 0;
  const size =
    input.outSize ??
    groundOutputSize(width, height, input.tilesX, input.tilesY, 0);
  const outW = size.width;
  const outH = size.height;
  const composed = new Uint8ClampedArray(outW * outH * 4);

  const borderIndices =
    input.lotBorderPatternIndices && input.lotBorderPatternIndices.length === 4
      ? input.lotBorderPatternIndices
      : [0, 0, 0, 0];
  const borderWidths =
    input.lotBorderWidths && input.lotBorderWidths.length === 4
      ? input.lotBorderWidths
      : [0, 0, 0, 0];
  /** 胜者图案格（主区 = LotColor.A / 边框带 = LotBorderColor.A），惰性切片缓存。 */
  const patternCellCache = new Map<number, Pixels | null>();
  function patternCell(index: number): Pixels | null {
    if (!useNormal) return null;
    const cell = index % 16;
    let cached = patternCellCache.get(cell);
    if (cached === undefined) {
      cached = copyAtlasRegion(input.normalAtlas!, cell, normalCellW, normalCellH);
      patternCellCache.set(cell, cached);
    }
    return cached;
  }
  /**
   * 图案格法线 → 坡度明暗（lot_composite `shade_of` 同款：平整 = 1.0，
   * 逆光面暗、向光面亮，近似引擎 lotCalcLighting 的图案光照）。
   */
  function shadeAt(cell: Pixels, u: number, v: number): number {
    const LIGHT_X = -0.5;
    const LIGHT_Y = -0.5;
    const STRENGTH = 1.4;
    const [nr, ng] = samplePatternBilinear(
      cell,
      u,
      v,
      input.tilesX,
      input.tilesY,
    );
    const nx = nr / 127.5 - 1;
    const ny = ng / 127.5 - 1;
    return Math.min(1.45, Math.max(0.55, 1 + STRENGTH * (nx * LIGHT_X + ny * LIGHT_Y)));
  }

  /** 画布 UV → raw mask 通道权重（双线性，= 引擎 GPU 采样口径）。
   *  边缘的亚 texel 平滑来自 mask 自带的软渐变坡；此前最近邻是 2026-09-13
   *  为抑制「高优通道外扩」改的，但外扩本就是引擎同款行为（GPU 双线性 +
   *  阈值 + 优先级链），最近邻的代价是斜边/曲线出现 4× mask texel 的
   *  阶梯锯齿（2026-09-18 用户反馈），故回归引擎口径。 */
  function sampleWeights(u: number, v: number): [number, number, number, number] {
    const raw = rawMask;
    if (!raw) return [0, 0, 0, 0];
    const fx = Math.min(Math.max(u * raw.width - 0.5, 0), raw.width - 1);
    const fy = Math.min(Math.max(v * raw.height - 0.5, 0), raw.height - 1);
    const x0 = Math.floor(fx);
    const y0 = Math.floor(fy);
    const x1 = Math.min(x0 + 1, raw.width - 1);
    const y1 = Math.min(y0 + 1, raw.height - 1);
    const tx = fx - x0;
    const ty = fy - y0;
    const rowA = y0 * raw.width;
    const rowB = y1 * raw.width;
    const out: [number, number, number, number] = [0, 0, 0, 0];
    for (let c = 0; c < 4; c += 1) {
      const a = raw.data[(rowA + x0) * 4 + c];
      const b = raw.data[(rowA + x1) * 4 + c];
      const top = a + (b - a) * tx;
      const cc = raw.data[(rowB + x0) * 4 + c];
      const d = raw.data[(rowB + x1) * 4 + c];
      const bottom = cc + (d - cc) * tx;
      out[c] = (top + (bottom - top) * ty) / 255;
    }
    return out;
  }

  for (let y = 0; y < outH; y += 1) {
    for (let x = 0; x < outW; x += 1) {
      const at = (y * outW + x) * 4;
      const u = (x + 0.5) / outW;
      const v = (y + 0.5) / outH;
      // 引擎优先级瀑布：w→z→y→x = A > B > G > R；8 级
      // A边框>A主色>B边框>B主色>…（addOverlay 逐字）。
      let channel = -1;
      let channelIsBorder = false;
      if (rawMask) {
        const weights = sampleWeights(u, v);
        for (const c of [3, 2, 1, 0]) {
          if (weights[c] > 0.5 - borderWidths[c]) {
            channel = c;
            channelIsBorder = weights[c] <= 0.5 + borderWidths[c];
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
            input.lotColors,
          );
        }
      }
      if (channel >= 0) {
        // 覆盖区 = 平色 × 图案坡度明暗（引擎不采样漫反射；明暗在线性空间
        // 相乘后回 sRGB = 探针 tinted() 同款）。边框色缺失回退浅灰 156
        //（= 后端 LotBorderColor 回退值）。
        const flat = channelIsBorder
          ? input.lotBorderColors?.[channel] ?? [156, 156, 156]
          : input.lotColors[channel];
        const cell = patternCell(
          channelIsBorder ? borderIndices[channel] : input.lotColors[channel][3],
        );
        const shade = cell ? shadeAt(cell, u, v) : 1;
        composed[at] = linearToSrgbByte(srgbToLinear(flat[0]) * shade);
        composed[at + 1] = linearToSrgbByte(srgbToLinear(flat[1]) * shade);
        composed[at + 2] = linearToSrgbByte(srgbToLinear(flat[2]) * shade);
      } else {
        // 未覆盖区 = 底图格整格拉伸（U 镜像）；引擎 overlayMask=0 无图案光照。
        if (input.baseTile) {
          const [br, bg, bb] = sampleBaseCell(input.baseTile, u, v);
          composed[at] = br;
          composed[at + 1] = bg;
          composed[at + 2] = bb;
        } else {
          composed[at] = 255;
          composed[at + 1] = 255;
          composed[at + 2] = 255;
        }
      }
      composed[at + 3] = 255;
    }
  }
  return { albedo: composed, width: outW, height: outH };
}

/** worker 消息协议（结构化克隆）。 */
export interface GroundComposeRequest {
  id: number;
  input: GroundComposeInput;
}
export interface GroundComposeResponse {
  id: number;
  albedo: Uint8ClampedArray<ArrayBuffer>;
  width: number;
  height: number;
}

/** Worker 消息端最小接口（避免引入 webworker lib 全局类型）。 */
interface ComposeWorkerScope {
  onmessage: ((event: MessageEvent<GroundComposeRequest>) => void) | null;
  postMessage(message: GroundComposeResponse | { id: number; error: string }): void;
}

/** Worker 入口（仅 worker 上下文执行）。 */
export function runGroundComposeWorker(scope: ComposeWorkerScope): void {
  scope.onmessage = (event: MessageEvent<GroundComposeRequest>) => {
    const { id, input } = event.data;
    try {
      const output = composeGroundPixels(input);
      scope.postMessage({ id, ...output });
    } catch (error) {
      scope.postMessage({
        id,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  };
}
