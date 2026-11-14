import { shallowRef } from "vue";
import { isTauri, tauriApi } from "@/api";

/** 游戏共享 Lot 地表图集（4×4 格，PNG base64）；全应用只读取一次。 */
const atlasPng = shallowRef<string | null>(null);
let request: Promise<void> | null = null;

export function useLotAtlas() {
  if (!request && isTauri() && typeof tauriApi.raster.readLotPreviewAtlases === "function") {
    request = tauriApi.raster
      .readLotPreviewAtlases()
      .then((value) => {
        atlasPng.value = value?.lotSurfacePng ?? null;
      })
      .catch(() => {
        atlasPng.value = null;
        request = null;
      });
  }
  return atlasPng;
}

const cellCache = new Map<string, Promise<string | null>>();

/** 切出图集第 index 格为独立 PNG data URL（供 CSS 平铺）。 */
export function atlasCellDataUrl(atlas: string, index: number): Promise<string | null> {
  const key = `url:${atlas.length}:${atlas.slice(0, 64)}:${index}`;
  let cached = cellCache.get(key);
  if (!cached) {
    cached = cropCell(atlas, index).then((cell) => cell?.toDataURL("image/png") ?? null);
    cellCache.set(key, cached);
  }
  return cached;
}

const imageCache = new Map<string, Promise<ImageData | null>>();

/** 切出图集第 index 格像素（供画布绘制区按格平铺回显）。 */
export function atlasCellImageData(
  atlas: string,
  index: number,
): Promise<ImageData | null> {
  const key = `px:${atlas.length}:${atlas.slice(0, 64)}:${index}`;
  let cached = imageCache.get(key);
  if (!cached) {
    cached = cropCell(atlas, index).then((cell) => {
      if (!cell) return null;
      const context = cell.getContext("2d");
      if (!context) return null;
      return context.getImageData(0, 0, cell.width, cell.height);
    });
    imageCache.set(key, cached);
  }
  return cached;
}

function cropCell(atlas: string, index: number): Promise<HTMLCanvasElement | null> {
  return new Promise((resolve) => {
    const image = new Image();
    image.onload = () => {
      const cellWidth = Math.floor(image.naturalWidth / 4);
      const cellHeight = Math.floor(image.naturalHeight / 4);
      const canvas = document.createElement("canvas");
      canvas.width = cellWidth;
      canvas.height = cellHeight;
      const context = canvas.getContext("2d");
      if (!context || cellWidth < 1 || cellHeight < 1) return resolve(null);
      context.drawImage(
        image,
        (index % 4) * cellWidth,
        Math.floor(index / 4) * cellHeight,
        cellWidth,
        cellHeight,
        0,
        0,
        cellWidth,
        cellHeight,
      );
      resolve(canvas);
    };
    image.onerror = () => resolve(null);
    image.src = `data:image/png;base64,${atlas}`;
  });
}

/** LotPatternGrid 同款雪碧图定位：显示图集第 index 格。 */
export function atlasCellStyle(atlas: string | null, index: number) {
  if (!atlas) return undefined;
  return {
    backgroundImage: `url(data:image/png;base64,${atlas})`,
    backgroundPosition: `${(index % 4) * 33.333333}% ${Math.floor(index / 4) * 33.333333}%`,
    backgroundSize: "400% 400%",
  };
}
