/**
 * 精细地面合成的**纯像素核心**（无 DOM 依赖）——主线程与 Worker 共用。
 *
 * 引擎语义（= lot_compose 探针 compose_tiles 同口径）：
 *  1. 选区：raw mask 四通道权重 >0.5 硬阈值，按引擎优先级链 A>B>G>R（w→z→y→x）
 *     选出唯一通道；边框带 = 权重 ∈ (0.5−bw, 0.5+bw]，着 LotBorderColor 平色；
 *  2. 材质：胜出通道铺 tile_{LotColor.A}（16 格共享图集），tile 按
 *     frac(uv × N) 平铺（N = LotSize / 0x0CCB7FD0，逐轴）；
 *  3. 着色：胜出通道 LotColor.RGB（后端已 sRGB 字节）乘 tile 原色，
 *     s10 染色图集 alpha 做亮度调制；
 *  4. 未覆盖区：铺底图格（图集 cell 8 草地；三楼对拍口径），不透明。
 *
 * 法线输出与反照率逐像素对齐（同格号、同平铺；仅未覆盖区改用底图格）。
 * 画布即引擎空间：mask 列序与模型 X 同向（lot_mask_alignment 裁定 identity）、
 * 行序翻转由后端统一完成，故 tile 采样 u/v 均直取（无镜像）。
 *
 * 着色规则：LotColor 属性缺失（authored=false）时回退色只是编辑器可视化，
 * 不参与着色——直接铺贴图原色（用户实测 2026-09-10）。
 */

export interface Pixels {
  data: Uint8ClampedArray<ArrayBuffer>;
  width: number;
  height: number;
}

export interface GroundComposeInput {
  /** 量化 mask 图（RGB = 通道色字节，alpha = 覆盖）。 */
  mask: Pixels;
  /** 原始通道权重图；缺失时回退量化图最近色硬分配。 */
  rawMask: Pixels | null;
  /** "Lot Textures" 地表共享纹理图集（4×4 格）。 */
  surface: Pixels | null;
  /** s10 染色图集（alpha 做亮度调制）。 */
  tintAtlas: Pixels | null;
  /** s15 法线图集（4×4 格）。 */
  normalAtlas: Pixels | null;
  /** 每通道材质源 tile（surface 切格或本地占位）；index 对应 LotColor[i]。 */
  channelTiles: (Pixels | null)[];
  /** 未覆盖区底图格（图集 cell 8）。 */
  defaultTile: Pixels | null;
  lotColors: [number, number, number, number][];
  lotColorsAuthored: boolean[];
  /** LotBorderColor1-4 的 sRGB RGB；null = 无边框带。 */
  lotBorderColors: [number, number, number][] | null;
  /** borderWidth1-4（边框带半宽，0..0.5）；null/全 0 = 无边框。 */
  lotBorderWidths: number[] | null;
  tilesX: number;
  tilesY: number;
}

export interface GroundComposeOutput {
  albedo: Uint8ClampedArray<ArrayBuffer>;
  normal: Uint8ClampedArray<ArrayBuffer> | null;
  width: number;
  height: number;
}

/** v1 回退路径的最近色硬分配（量化 mask RGB → 通道下标）。 */
export function nearestChannel(
  r: number,
  g: number,
  b: number,
  colors: [number, number, number, number][],
): number {  let best = -1;
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

/** 平铺采样：引擎 frac(baseUV)——UV 跨 0..N 该格重复 N 次。 */
function sampleTiled(
  source: Pixels,
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

/** 染色图集 alpha（0..1），平铺口径同底图。 */
function sampleTiledAlpha(
  source: Pixels,
  u: number,
  v: number,
  tilesX: number,
  tilesY: number,
): number {
  const fu = u * tilesX;
  const fv = v * tilesY;
  const px = Math.min(source.width - 1, Math.floor((fu - Math.floor(fu)) * source.width));
  const py = Math.min(source.height - 1, Math.floor((fv - Math.floor(fv)) * source.height));
  return source.data[(py * source.width + px) * 4 + 3] / 255;
}

/**
 * 合成主循环（纯函数）。4× 超采样输出：mask 权重双线性插值 + tile 原生
 * 分辨率采样，消除 128px 权重图直贴 64m 地面的模糊（对拍 2026-09-12）。
 */
export function composeGroundPixels(input: GroundComposeInput): GroundComposeOutput {
  const { mask, rawMask } = input;
  const width = mask.width;
  const height = mask.height;
  // 4× 超采样输出上限 1024²（与既有口径一致）。
  const scale = Math.min(4, Math.max(1, Math.floor(1024 / Math.max(width, height))));
  const outW = width * scale;
  const outH = height * scale;
  const composed = new Uint8ClampedArray(outW * outH * 4);
  const useNormal = input.normalAtlas !== null &&
    Math.floor(input.normalAtlas.width / 4) >= 1 &&
    Math.floor(input.normalAtlas.height / 4) >= 1;
  const normalCellW = useNormal ? Math.floor(input.normalAtlas!.width / 4) : 0;
  const normalCellH = useNormal ? Math.floor(input.normalAtlas!.height / 4) : 0;
  const normal = useNormal ? new Uint8ClampedArray(outW * outH * 4) : null;
  const tintCellW = input.tintAtlas ? Math.floor(input.tintAtlas.width / 4) : 0;
  const tintCellH = input.tintAtlas ? Math.floor(input.tintAtlas.height / 4) : 0;
  const tintCells = input.tintAtlas
    ? input.lotColors.map((color) =>
        copyAtlasRegion(input.tintAtlas!, color[3] % 16, tintCellW, tintCellH),
      )
    : null;
  const normalCells = useNormal
    ? input.lotColors.map((color) =>
        copyAtlasRegion(input.normalAtlas!, color[3] % 16, normalCellW, normalCellH),
      )
    : null;
  const normalDefault = useNormal
    ? copyAtlasRegion(input.normalAtlas!, 8, normalCellW, normalCellH)
    : null;

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
      const u = x / outW;
      const v = y / outH;
      // 引擎优先级链：w→z→y→x = A > B > G > R；8 级瀑布
      // A边框>A主色>B边框>B主色>…（addOverlay 逐字）。
      let channel = -1;
      let channelIsBorder = false;
      const borderWidths =
        input.lotBorderWidths && input.lotBorderWidths.length === 4
          ? input.lotBorderWidths
          : [0, 0, 0, 0];
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
      const source = channel >= 0 ? input.channelTiles[channel] : input.defaultTile;
      if (channel >= 0 && channelIsBorder && input.lotBorderColors?.[channel]) {
        // 边框带 = LotBorderColor 平色（引擎里图案进法线，不进反照率）。
        const border = input.lotBorderColors[channel]!;
        composed[at] = border[0];
        composed[at + 1] = border[1];
        composed[at + 2] = border[2];
      } else if (source) {
        const [tr, tg, tb] = sampleTiled(source, u, v, input.tilesX, input.tilesY);
        const tint =
          channel >= 0 && input.lotColorsAuthored[channel]
            ? [input.lotColors[channel][0], input.lotColors[channel][1], input.lotColors[channel][2]]
            : [255, 255, 255];
        let r8 = (tr * tint[0]) / 255;
        let g8 = (tg * tint[1]) / 255;
        let b8 = (tb * tint[2]) / 255;
        // s10 染色图集调制：车辙/铺装的明暗细节在此（覆盖区 overlayMask=1）。
        if (channel >= 0 && tintCells?.[channel]) {
          const mul = Math.min(2, sampleTiledAlpha(tintCells[channel]!, u, v, input.tilesX, input.tilesY) * 2);
          r8 = Math.min(255, r8 * mul);
          g8 = Math.min(255, g8 * mul);
          b8 = Math.min(255, b8 * mul);
        }
        composed[at] = r8;
        composed[at + 1] = g8;
        composed[at + 2] = b8;
      } else {
        composed[at] = 58;
        composed[at + 1] = 62;
        composed[at + 2] = 54;
      }
      // 法线：同格（覆盖区 = 胜出通道 LotColor.A，未覆盖区 = 底图格）、同平铺
      // 频率——与反照率逐像素对齐。无格可采时退化为平坦法线 (128,128,255)。
      if (normal) {
        const normalSource =
          channel >= 0 ? normalCells?.[channel] ?? null : normalDefault;
        if (normalSource) {
          const [nr, ng, nb] = sampleTiled(normalSource, u, v, input.tilesX, input.tilesY);
          normal[at] = nr;
          normal[at + 1] = ng;
          normal[at + 2] = nb;
        } else {
          normal[at] = 128;
          normal[at + 1] = 128;
          normal[at + 2] = 255;
        }
        normal[at + 3] = 255;
      }
      composed[at + 3] = 255;
    }
  }
  return { albedo: composed, normal, width: outW, height: outH };
}

/** worker 消息协议（结构化克隆）。 */
export interface GroundComposeRequest {
  id: number;
  input: GroundComposeInput;
}
export interface GroundComposeResponse {
  id: number;
  albedo: Uint8ClampedArray<ArrayBuffer>;
  normal: Uint8ClampedArray<ArrayBuffer> | null;
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
