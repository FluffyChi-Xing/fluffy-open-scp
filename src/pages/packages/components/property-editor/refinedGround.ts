import type * as ThreeNamespace from "three";

/**
 * 精细渲染的 LotMask 地面合成（引擎语义近似）：
 * mask 四通道选区 → LotColor1-4；每通道 = LotColor.RGB 着色 ×
 * LotColor.A 索引的地面贴图（16 格图集，对应 shader 的
 * baseTileUVMinMax 4×4；C# lot editor 的 GroundTexture 下拉读的
 * 就是这个 Alpha）。
 *
 * 着色规则：LotColor 属性缺失（authored=false）时回退色（黑/红/绿/蓝）
 * 只是编辑器可视化，不参与着色——直接铺贴图原色，避免黑块/纯红块
 * （用户实测 2026-09-10）。未选中区域铺暗化底图避免局部透明不可见。
 */

const groundTextureUrls = import.meta.glob<{ default: string }>(
  "../../../../assets/ground/*.png",
  { eager: true, import: "default", query: "?url" },
) as unknown as Record<string, string>;

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

/**
 * v4（2026-09-12 引擎语义落地）：LotMask 原始通道权重图（未阈值化，
 * RGBA 字节 = LotColor1-4 覆盖权重 0-255 渐变）+ 每通道材质
 * （tile_{LotColor.A} × LotColor.RGB）做 dot(colors, masks) 线性软混合
 * ——与引擎 shader 完全同构。边界过渡带自然软化；弱覆盖区（门前
 * 绿地等）不再被硬阈值丢弃。surface（"Lot Textures" 4×4 通用图集）
 * 就绪时用作 tile 源，否则回退本地占位 tile（assets/ground）。
 */
export async function composeRefinedGround(
  lotColors: [number, number, number, number][],
  lotColorsAuthored: boolean[],
  maskImage: TexImageSource,
  THREE: typeof ThreeNamespace,
  surface?: ImageData | null,
  /** v4：原始通道权重图；缺失时回退 v1（量化图最近色硬分配）。 */
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

  /** 归一化 lot UV → raw mask 通道权重的双线性采样。 */
  function sampleWeights(u: number, v: number): [number, number, number, number] {
    const raw = rawMask;
    if (!raw) return [0, 0, 0, 0];
    const fx = u * (raw.width - 1);
    const fy = v * (raw.height - 1);
    const x0 = Math.floor(fx);
    const y0 = Math.floor(fy);
    const x1 = Math.min(x0 + 1, raw.width - 1);
    const y1 = Math.min(y0 + 1, raw.height - 1);
    const tx = fx - x0;
    const ty = fy - y0;
    const read = (px: number, py: number): [number, number, number, number] => {
      const at = (py * raw.width + px) * 4;
      return [
        raw.data[at] / 255,
        raw.data[at + 1] / 255,
        raw.data[at + 2] / 255,
        raw.data[at + 3] / 255,
      ];
    };
    const c00 = read(x0, y0);
    const c10 = read(x1, y0);
    const c01 = read(x0, y1);
    const c11 = read(x1, y1);
    const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
    return [0, 1, 2, 3].map((channel) => {
      const top = lerp(c00[channel], c10[channel], tx);
      const bottom = lerp(c01[channel], c11[channel], tx);
      return lerp(top, bottom, ty);
    }) as [number, number, number, number];
  }
  // 每通道材质源：surface 图集 tile（按 LotColor.A）优先，本地占位回退
  const channelTiles = await Promise.all(
    lotColors.map((color) => {
      if (useSurface) {
        const index = color[3] % 16;
        const originX = (index % 4) * tileW;
        const originY = Math.floor(index / 4) * tileH;
        return Promise.resolve(
          context.createImageData(tileW, tileH) &&
            copyRegion(surface!, originX, originY, tileW, tileH),
        );
      }
      return tile(color[3] ?? 0);
    }),
  );
  const fallbackTile = await tile(0);
  // 空腔默认材质 = 草地（图集 tile 8；豪宅实证：mask 全空的 lot 在游戏
  // 中显示草地庭院，环形车道为模型自带几何）
  const defaultTile = useSurface
    ? copyRegionFromSurface(8)
    : await tile(8);

  /** 从 surface 图集复制第 index 格（4×4）。 */
  function copyRegionFromSurface(index: number): ImageData | null {
    if (!surface) return null;
    const originX = (index % 4) * tileW;
    const originY = Math.floor(index / 4) * tileH;
    return copyRegion(surface, originX, originY, tileW, tileH);
  }

  /** 从图集 ImageData 复制一个 tile 区域。 */
  function copyRegion(
    source: ImageData,
    originX: number,
    originY: number,
    w: number,
    h: number,
  ): ImageData | null {
    const out = new ImageData(w, h);
    for (let y = 0; y < h; y += 1) {
      const srcRow = ((originY + y) * source.width + originX) * 4;
      out.data.set(source.data.subarray(srcRow, srcRow + w * 4), y * w * 4);
    }
    return out;
  }

  /**
   * 采样一张 tile：整图单次映射（引擎 shader
   * lotTexCoord = lerp(lotBaseUVMin, lotBaseUVMax, uv) 语义——tile 是
   * 该 lot 的完整地面规划图，无平铺；8×8 平铺为已证伪的旧假设）。
   */
  function sampleTile(
    source: ImageData,
    x: number,
    y: number,
  ): [number, number, number] {
    const px = Math.min(source.width - 1, Math.floor((x / outW) * source.width));
    const py = Math.min(source.height - 1, Math.floor((y / outH) * source.height));
    const offset = (py * source.width + px) * 4;
    return [source.data[offset], source.data[offset + 1], source.data[offset + 2]];
  }

  // 主导通道（覆盖总量最大者）：空腔区延伸其材质（游戏里空腔处显示
  // 地形材质的延续，而非黑斑）。
  let dominantChannel = 0;
  if (rawMask) {
    let best = -1;
    for (let channel = 0; channel < 4; channel += 1) {
      let sum = 0;
      for (let at = channel; at < rawMask.data.length; at += 4) sum += rawMask.data[at];
      if (sum > best) {
        best = sum;
        dominantChannel = channel;
      }
    }
  }

  for (let y = 0; y < outH; y += 1) {
    for (let x = 0; x < outW; x += 1) {
      const at = (y * outW + x) * 4;
      const u = x / outW;
      const v = y / outH;
      if (rawMask) {
        // v4：dot(colors, masks) 线性软混合（引擎 shader 同构）。
        // 权重双线性插值；Σw≈0 的空腔区延伸主导通道材质（不暗化——
        // 游戏里空腔处是地形延续，黑斑观感错误）。
        const weights = sampleWeights(u, v);
        const weightSum = weights.reduce((sum, value) => sum + value, 0);
        let channel = dominantChannel;
        let source: ImageData | null;
        if (weightSum < 0.04) {
          source = defaultTile;
        } else {
          let best = -1;
          for (let c = 0; c < 4; c += 1) {
            if (weights[c] > best) {
              best = weights[c];
              channel = c;
            }
          }
          source = channelTiles[channel];
        }
        if (source) {
          const [tr, tg, tb] = sampleTile(source, x, y);
          const [cr, cg, cb] = lotColorsAuthored[channel]
            ? [lotColors[channel][0], lotColors[channel][1], lotColors[channel][2]]
            : [255, 255, 255];
          composed.data[at] = (tr * cr) / 255;
          composed.data[at + 1] = (tg * cg) / 255;
          composed.data[at + 2] = (tb * cb) / 255;
        } else {
          composed.data[at] = 58;
          composed.data[at + 1] = 62;
          composed.data[at + 2] = 54;
        }
        composed.data[at + 3] = 255;
        continue;
      }
      // v1 回退：量化图最近色硬分配（raw 缺失时的兜底）。
      const mx = Math.min(width - 1, Math.floor(u * width));
      const my = Math.min(height - 1, Math.floor(v * height));
      const mat = (my * width + mx) * 4;
      const alpha = mask.data[mat + 3];
      let source: ImageData | null = fallbackTile;
      let tint: [number, number, number] | null = null;
      let shade = 0.55;
      if (alpha >= 16) {
        const channel = nearestChannel(
          mask.data[mat],
          mask.data[mat + 1],
          mask.data[mat + 2],
          lotColors,
        );
        source = channelTiles[channel];
        shade = 1;
        if (lotColorsAuthored[channel]) {
          tint = [lotColors[channel][0], lotColors[channel][1], lotColors[channel][2]];
        }
      }
      if (source) {
        const [tr, tg, tb] = sampleTile(source, x, y);
        composed.data[at] = ((tr * (tint ? tint[0] : 255)) / 255) * shade;
        composed.data[at + 1] = ((tg * (tint ? tint[1] : 255)) / 255) * shade;
        composed.data[at + 2] = ((tb * (tint ? tint[2] : 255)) / 255) * shade;
        composed.data[at + 3] = 255;
      } else {
        composed.data[at] = 58 * shade;
        composed.data[at + 1] = 62 * shade;
        composed.data[at + 2] = 54 * shade;
        composed.data[at + 3] = 255;
      }
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
