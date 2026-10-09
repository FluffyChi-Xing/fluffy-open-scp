/** CPU composition of lot albedo and tangent normals; lighting stays dynamic. */
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
  baseTileIndex?: number;
  lotColorHeights?: number[] | null;
  lotBorderHeights?: number[] | null;
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
  normal: Uint8ClampedArray<ArrayBuffer>;
  width: number;
  height: number;
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
 * 底图格采样（双线性 wrap，**按周期平铺**）。
 *
 * generic_lot 的 baseUV 来自 texcoord0.zw，并以 frac(baseUV) 平铺；
 * CreateUnitLotGraphics 按 0x0CCB7FD0 指定的米制周期生成该坐标。
 */
function sampleBaseCell(
  source: Pixels,
  u: number,
  v: number,
  tilesX: number,
  tilesY: number,
): [number, number, number] {
  return samplePatternBilinear(source, u, v, tilesX, tilesY);
}

/**
 * 图案格平铺采样（双线性，wrap）：引擎 U 沿 +X，V 沿 -Y。
 * 输出行 v 沿 +Y（遮罩已由后端翻行），图集仍保留原始行序。
 * 平铺重复之间无缝 wrap；输出密度≈格原生密度（1:1）时退化为最近邻
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
  const fy = frac((0.5 - v) * tilesY) * source.height - 0.5;
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
  const wx = wrap(x0);
  const wy = wrapY(y0);
  const topLeft = (wy * source.width + wx) * 4;
  const topRight = (wy * source.width + x1) * 4;
  const bottomLeft = (y1 * source.width + wx) * 4;
  const bottomRight = (y1 * source.width + x1) * 4;
  const data = source.data;
  const out: [number, number, number] = [0, 0, 0];
  for (let c = 0; c < 3; c += 1) {
    const top = data[topLeft + c] + (data[topRight + c] - data[topLeft + c]) * tx;
    const bottom = data[bottomLeft + c] + (data[bottomRight + c] - data[bottomLeft + c]) * tx;
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
  const normals = new Uint8ClampedArray(composed.length);

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

  // overlayGetHeight: ordered channel blending, including the authored border
  // height. Bake fwidth at output texel resolution; quarter-mask-texel offsets
  // match the HLSL normal reconstruction (samplerInfo5.zw * .25).
  const edge = 7 * Math.max(1 / outW, 1 / outH) * 1.5;
  const smooth = (center: number, value: number): number => {
    const lo = Math.max(center - edge, 0.05);
    const hi = Math.min(center + edge, 0.95);
    const t = Math.min(1, Math.max(0, (value - lo) / Math.max(hi - lo, 1e-6)));
    return t * t * (3 - 2 * t);
  };
  const hasHeight = !!rawMask && [...(input.lotColorHeights ?? []), ...(input.lotBorderHeights ?? [])].some(h => h !== 0);
  function heightAt(u: number, v: number): number {
    const weights = sampleWeights(u, v);
    let height = 0;
    for (let c = 0; c < 4; c++) {
      const mask = smooth(0.5 - borderWidths[c], weights[c]);
      const border = 1 - smooth(0.5 + borderWidths[c], weights[c]);
      const main = input.lotColorHeights?.[c] ?? 0;
      const h = main + ((input.lotBorderHeights?.[c] ?? 0) - main) * border;
      height += (h - height) * mask;
    }
    return height;
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
      let normalCell = patternCell(input.baseTileIndex ?? 8);
      if (channel >= 0) {
        // 覆盖区使用主色或边框固有色，图案和高度法线单独输出供动态光照。
        // 边框色缺失回退浅灰 156（后端 LotBorderColor 默认值）。
        const flat = channelIsBorder
          ? input.lotBorderColors?.[channel] ?? [156, 156, 156]
          : input.lotColors[channel];
        normalCell = patternCell(
          channelIsBorder ? borderIndices[channel] : input.lotColors[channel][3],
        );
        composed[at] = flat[0];
        composed[at + 1] = flat[1];
        composed[at + 2] = flat[2];
      } else {
        // 未覆盖区使用底图漫反射和 baseTileIndex 指向的图案法线。
        if (input.baseTile) {
          const [br, bg, bb] = sampleBaseCell(
            input.baseTile,
            u,
            v,
            input.tilesX,
            input.tilesY,
          );
          composed[at] = br;
          composed[at + 1] = bg;
          composed[at + 2] = bb;
        } else {
          composed[at] = 255;
          composed[at + 1] = 255;
          composed[at + 2] = 255;
        }
      }
      let nx = 0, ny = 0, nz = 1;
      if (normalCell) {
        const n = samplePatternBilinear(normalCell, u, v, input.tilesX, input.tilesY);
        nx = n[0] / 127.5 - 0.9985;
        ny = n[1] / 127.5 - 0.9985;
        nz = n[2] / 127.5 - 0.9985;
      }
      if (hasHeight && channel >= 0) {
        const du = 0.25 / rawMask!.width, dv = 0.25 / rawMask!.height;
        nx += heightAt(u - du, v) - heightAt(u + du, v);
        ny += heightAt(u, v - dv) - heightAt(u, v + dv);
      }
      const length = Math.hypot(nx, ny, nz) || 1;
      normals[at] = (nx / length * 0.5 + 0.5) * 255;
      normals[at + 1] = (ny / length * 0.5 + 0.5) * 255;
      normals[at + 2] = (nz / length * 0.5 + 0.5) * 255;
      normals[at + 3] = 255;
      composed[at + 3] = 255;
    }
  }
  return { albedo: composed, normal: normals, width: outW, height: outH };
}

/** worker 消息协议（结构化克隆）。 */
export interface GroundComposeRequest {
  id: number;
  input: GroundComposeInput;
}
export interface GroundComposeResponse {
  id: number;
  albedo: Uint8ClampedArray<ArrayBuffer>;
  normal: Uint8ClampedArray<ArrayBuffer>;
  width: number;
  height: number;
}

/** Worker 消息端最小接口（避免引入 webworker lib 全局类型）。 */
interface ComposeWorkerScope {
  onmessage: ((event: MessageEvent<GroundComposeRequest>) => void) | null;
  postMessage(message: GroundComposeResponse | { id: number; error: string }, transfer?: Transferable[]): void;
}

/** Worker 入口（仅 worker 上下文执行）。 */
export function runGroundComposeWorker(scope: ComposeWorkerScope): void {
  scope.onmessage = (event: MessageEvent<GroundComposeRequest>) => {
    const { id, input } = event.data;
    try {
      const output = composeGroundPixels(input);
      scope.postMessage({ id, ...output }, [output.albedo.buffer, output.normal.buffer]);
    } catch (error) {
      scope.postMessage({
        id,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  };
}
