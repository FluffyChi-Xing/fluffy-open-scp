/**
 * RasterDocument：绘制器的像素文档模型（框架无关）。
 *
 * 统一数据模型是 RGBA8 像素缓冲（Uint8ClampedArray, w×h×4）。
 * Decal / LotMask 等"四通道语义图"由模式层在工具写入时做约束，
 * 文档层只负责像素的存取、区域拷贝与脏区追踪。
 */

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export class RasterDocument {
  readonly width: number;
  readonly height: number;
  readonly pixels: Uint8ClampedArray;

  constructor(width: number, height: number, rgba?: Uint8ClampedArray) {
    if (width <= 0 || height <= 0) {
      throw new Error("raster dimensions must be positive");
    }
    this.width = width;
    this.height = height;
    if (rgba) {
      const needed = width * height * 4;
      if (rgba.length < needed) {
        throw new Error("pixel buffer smaller than document size");
      }
      this.pixels = rgba;
    } else {
      this.pixels = new Uint8ClampedArray(width * height * 4);
    }
  }

  inside(x: number, y: number): boolean {
    return x >= 0 && y >= 0 && x < this.width && y < this.height;
  }

  getPixel(x: number, y: number): [number, number, number, number] {
    const base = (y * this.width + x) * 4;
    return [
      this.pixels[base],
      this.pixels[base + 1],
      this.pixels[base + 2],
      this.pixels[base + 3],
    ];
  }

  setPixel(
    x: number,
    y: number,
    rgba: readonly [number, number, number, number],
  ): void {
    if (!this.inside(x, y)) return;
    const base = (y * this.width + x) * 4;
    this.pixels[base] = rgba[0];
    this.pixels[base + 1] = rgba[1];
    this.pixels[base + 2] = rgba[2];
    this.pixels[base + 3] = rgba[3];
  }

  copyRegion(rect: Rect): Uint8ClampedArray {
    const out = new Uint8ClampedArray(rect.w * rect.h * 4);
    for (let row = 0; row < rect.h; row += 1) {
      const srcBase = ((rect.y + row) * this.width + rect.x) * 4;
      out.set(
        this.pixels.subarray(srcBase, srcBase + rect.w * 4),
        row * rect.w * 4,
      );
    }
    return out;
  }

  restoreRegion(rect: Rect, data: Uint8ClampedArray): void {
    for (let row = 0; row < rect.h; row += 1) {
      const dstBase = ((rect.y + row) * this.width + rect.x) * 4;
      this.pixels.set(
        data.subarray(row * rect.w * 4, (row + 1) * rect.w * 4),
        dstBase,
      );
    }
  }

  fullRect(): Rect {
    return { x: 0, y: 0, w: this.width, h: this.height };
  }
}

/** 归一化脏矩形：合并任意点/区域集合的最小包围盒。 */
export function unionRect(a: Rect | null, b: Rect): Rect {
  if (!a) return { ...b };
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return {
    x,
    y,
    w: Math.max(a.x + a.w, b.x + b.w) - x,
    h: Math.max(a.y + a.h, b.y + b.h) - y,
  };
}

/** 把任意点列收缩为包围盒（越界点会被 clamp 后参与，调用方自行过滤）。 */
export function rectOfPoints(points: { x: number; y: number }[]): Rect | null {
  if (!points.length) return null;
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const x = Math.min(...xs);
  const y = Math.min(...ys);
  return { x, y, w: Math.max(...xs) - x + 1, h: Math.max(...ys) - y + 1 };
}
