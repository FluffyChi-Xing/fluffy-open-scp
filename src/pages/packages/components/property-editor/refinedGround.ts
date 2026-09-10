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

/** 每边平铺的贴图重复数（引擎为 baseTileUVMinMax 图集裁剪，此处近似）。 */
const TILES_PER_EDGE = 8;

export async function composeRefinedGround(
  lotColors: [number, number, number, number][],
  lotColorsAuthored: boolean[],
  maskImage: TexImageSource,
  THREE: typeof ThreeNamespace,
): Promise<ThreeNamespace.CanvasTexture | null> {
  const image = maskImage as { width?: number; height?: number };
  const width = image.width ?? 0;
  const height = image.height ?? 0;
  if (!width || !height) return null;
  const tiles = await Promise.all(
    lotColors.map((color) => tile(color[3] ?? 0)),
  );
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) return null;
  context.drawImage(maskImage as CanvasImageSource, 0, 0);
  const mask = context.getImageData(0, 0, width, height);
  const composed = context.createImageData(width, height);
  // 未选中区域铺默认底图（tile 0 × 0.55 暗化），避免地面局部透明不可见。
  const baseTile = await tile(0);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const at = (y * width + x) * 4;
      const alpha = mask.data[at + 3];
      let source: ImageData | null = baseTile;
      let tint: [number, number, number] | null = null;
      let shade = 0.55;
      if (alpha >= 16) {
        const channel = nearestChannel(
          mask.data[at],
          mask.data[at + 1],
          mask.data[at + 2],
          lotColors,
        );
        source = tiles[channel];
        shade = 1;
        if (lotColorsAuthored[channel]) {
          tint = [lotColors[channel][0], lotColors[channel][1], lotColors[channel][2]];
        }
      }
      if (source) {
        const tileX = Math.floor((x / width) * TILES_PER_EDGE * source.width) % source.width;
        const tileY = Math.floor((y / height) * TILES_PER_EDGE * source.height) % source.height;
        const offset = (tileY * source.width + tileX) * 4;
        composed.data[at] = ((source.data[offset] * (tint ? tint[0] : 255)) / 255) * shade;
        composed.data[at + 1] = ((source.data[offset + 1] * (tint ? tint[1] : 255)) / 255) * shade;
        composed.data[at + 2] = ((source.data[offset + 2] * (tint ? tint[2] : 255)) / 255) * shade;
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
  texture.minFilter = THREE.LinearFilter;
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
