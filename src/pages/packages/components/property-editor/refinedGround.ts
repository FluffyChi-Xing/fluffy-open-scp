import type * as ThreeNamespace from "three";

/**
 * 精细渲染的 LotMask 地面合成（引擎语义近似）：
 * mask 四通道选区 → LotColor1-4；每通道 = LotColor.RGB 着色 ×
 * LotColor.A 索引的地面贴图（16 格图集，对应 shader 的
 * baseTileUVMinMax 4×4；C# lot editor 的 GroundTexture 下拉读的
 * 就是这个 Alpha）。未选中像素保持透明。
 */

const groundTextureUrls = import.meta.glob<{ default: string }>(
  "../../../assets/ground/*.png",
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
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const at = (y * width + x) * 4;
      const alpha = mask.data[at + 3];
      if (alpha < 16) continue;
      const channel = nearestChannel(
        mask.data[at],
        mask.data[at + 1],
        mask.data[at + 2],
        lotColors,
      );
      const source = tiles[channel];
      const [tr, tg, tb] = lotColors[channel];
      if (source) {
        const tileX = Math.floor((x / width) * TILES_PER_EDGE * source.width) % source.width;
        const tileY = Math.floor((y / height) * TILES_PER_EDGE * source.height) % source.height;
        const offset = (tileY * source.width + tileX) * 4;
        composed.data[at] = (source.data[offset] * tr) / 255;
        composed.data[at + 1] = (source.data[offset + 1] * tg) / 255;
        composed.data[at + 2] = (source.data[offset + 2] * tb) / 255;
        composed.data[at + 3] = 255;
      } else {
        composed.data[at] = tr;
        composed.data[at + 1] = tg;
        composed.data[at + 2] = tb;
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
