import type { RasterDocument } from "./document";
import { unionRect, type Rect } from "./document";

export type Rgba = readonly [number, number, number, number];

/**
 * 绘制工具的像素算法（框架无关）：全部"以回调写像素"实现，
 * 便于在 document 与拖拽预览层之间复用。
 */
export type PlotFn = (x: number, y: number) => void;

/** 以圆形笔刷端点盖章。 */
export function stampBrush(
  doc: RasterDocument,
  x: number,
  y: number,
  size: number,
  rgba: Rgba,
  plot: PlotFn,
): void {
  const radius = Math.max(0, (size - 1) / 2);
  const center = size / 2;
  const r2 = (radius + 0.25) ** 2;
  for (let dy = 0; dy < size; dy += 1) {
    for (let dx = 0; dx < size; dx += 1) {
      const px = x - Math.floor(center) + dx;
      const py = y - Math.floor(center) + dy;
      const dist2 = (px + 0.5 - (x + 0.5)) ** 2 + (py + 0.5 - (y + 0.5)) ** 2;
      if (dist2 <= r2 || size <= 2) {
        if (doc.inside(px, py)) {
          doc.setPixel(px, py, rgba);
          plot(px, py);
        }
      }
    }
  }
}

/** 笔刷沿折线路径连续盖章（Bresenham 粗线，线段间无缝）。 */
export function strokePath(
  doc: RasterDocument,
  points: { x: number; y: number }[],
  size: number,
  rgba: Rgba,
  plot: PlotFn,
): Rect | null {
  let dirty: Rect | null = null;
  if (points.length === 0) return null;
  if (points.length === 1) {
    stampBrush(doc, points[0].x, points[0].y, size, rgba, plot);
    return {
      x: points[0].x - size,
      y: points[0].y - size,
      w: size * 2,
      h: size * 2,
    };
  }
  for (let index = 1; index < points.length; index += 1) {
    const from = points[index - 1];
    const to = points[index];
    dirty = unionRect(dirty, bresenham(doc, from, to, size, rgba, plot));
  }
  return dirty;
}

function bresenham(
  doc: RasterDocument,
  from: { x: number; y: number },
  to: { x: number; y: number },
  size: number,
  rgba: Rgba,
  plot: PlotFn,
): Rect {
  let dirty: Rect | null = null;
  let x0 = from.x;
  let y0 = from.y;
  const dx = Math.abs(to.x - x0);
  const dy = -Math.abs(to.y - y0);
  const stepX = x0 < to.x ? 1 : -1;
  const stepY = y0 < to.y ? 1 : -1;
  let err = dx + dy;
  for (;;) {
    stampBrush(doc, x0, y0, size, rgba, plot);
    dirty = unionRect(dirty, {
      x: x0 - size,
      y: y0 - size,
      w: size * 2,
      h: size * 2,
    });
    if (x0 === to.x && y0 === to.y) break;
    const e2 = err * 2;
    if (e2 >= dy) {
      err += dy;
      x0 += stepX;
    }
    if (e2 <= dx) {
      err += dx;
      y0 += stepY;
    }
  }
  return dirty ?? { x: 0, y: 0, w: 0, h: 0 };
}

/** 矩形（空心/实心）。 */
export function drawRect(
  doc: RasterDocument,
  from: { x: number; y: number },
  to: { x: number; y: number },
  size: number,
  rgba: Rgba,
  fill: boolean,
  plot: PlotFn,
): Rect | null {
  const x0 = Math.min(from.x, to.x);
  const y0 = Math.min(from.y, to.y);
  const x1 = Math.max(from.x, to.x);
  const y1 = Math.max(from.y, to.y);
  let dirty: Rect | null = null;
  for (let y = y0; y <= y1; y += 1) {
    for (let x = x0; x <= x1; x += 1) {
      const onEdge = x === x0 || x === x1 || y === y0 || y === y1;
      if (fill || onEdge) {
        doc.setPixel(x, y, rgba);
        plot(x, y);
        dirty = unionRect(dirty, { x, y, w: 1, h: 1 });
      }
    }
  }
  void size;
  return dirty;
}

/**
 * 扫描线油漆桶：RGBA 欧氏距离容差的连通区域填充。
 * tolerance = 0 时为精确匹配；>0 时种子颜色 4 通道距离 ≤ tolerance 的像素
 * 视为同一区域（外部图片的抗锯齿渐变区域需要非零容差才能整片填充）。
 */
export function floodFill(
  doc: RasterDocument,
  x: number,
  y: number,
  rgba: Rgba,
  plot: PlotFn,
  tolerance = 0,
): Rect | null {
  if (!doc.inside(x, y)) return null;
  const target = doc.getPixel(x, y);
  if (
    target[0] === rgba[0] &&
    target[1] === rgba[1] &&
    target[2] === rgba[2] &&
    target[3] === rgba[3]
  ) {
    return null;
  }
  const limit = tolerance * tolerance;
  const match = (px: readonly [number, number, number, number]) => {
    if (limit === 0) {
      return (
        px[0] === target[0] &&
        px[1] === target[1] &&
        px[2] === target[2] &&
        px[3] === target[3]
      );
    }
    const dr = px[0] - target[0];
    const dg = px[1] - target[1];
    const db = px[2] - target[2];
    const da = px[3] - target[3];
    return dr * dr + dg * dg + db * db + da * da <= limit;
  };
  let dirty: Rect | null = null;
  const stack: [number, number][] = [[x, y]];
  const visited = new Set<number>();
  while (stack.length) {
    const [cx, cy] = stack.pop()!;
    let west = cx;
    let east = cx;
    while (west > 0 && match(doc.getPixel(west - 1, cy))) west -= 1;
    while (east < doc.width - 1 && match(doc.getPixel(east + 1, cy))) east += 1;
    for (let px = west; px <= east; px += 1) {
      const key = cy * doc.width + px;
      if (visited.has(key)) continue;
      visited.add(key);
      doc.setPixel(px, cy, rgba);
      plot(px, cy);
      dirty = unionRect(dirty, { x: px, y: cy, w: 1, h: 1 });
      const north = cy - 1;
      const south = cy + 1;
      if (
        north >= 0 &&
        match(doc.getPixel(px, north)) &&
        !visited.has(north * doc.width + px)
      ) {
        stack.push([px, north]);
      }
      if (
        south < doc.height &&
        match(doc.getPixel(px, south)) &&
        !visited.has(south * doc.width + px)
      ) {
        stack.push([px, south]);
      }
    }
  }
  return dirty ?? { x: 0, y: 0, w: 0, h: 0 };
}

export interface ParkingRowOptions {
  /** 每条停车线的长度（垂直于拖拽方向，px）。 */
  length: number;
  /** 相邻停车线沿拖拽方向的间距（px）。 */
  spacing: number;
}

/**
 * 停车位笔刷：沿 from→to 连线每隔 spacing 画一条垂直短线（朝拖拽方向
 * 右侧延伸），模拟一排等距停车格隔线。参数默认值取自游戏原生标线
 * 0x7BE85E77 的实测（线宽 2px / 线长 13px / 间距 6px，0.75 m/px）。
 */
export function strokeParkingRow(
  doc: RasterDocument,
  from: { x: number; y: number },
  to: { x: number; y: number },
  size: number,
  rgba: Rgba,
  options: ParkingRowOptions,
  plot: PlotFn,
): Rect | null {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const span = Math.hypot(dx, dy);
  if (span < 1 || options.spacing < 1 || options.length < 1) return null;
  // 拖拽方向单位向量与右侧法线（屏幕 y 向下）
  const ux = dx / span;
  const uy = dy / span;
  const nx = -uy;
  const ny = ux;
  let dirty: Rect | null = null;
  for (let t = 0; t <= span; t += options.spacing) {
    const cx = from.x + ux * t;
    const cy = from.y + uy * t;
    const tick = strokePath(
      doc,
      [
        { x: Math.round(cx), y: Math.round(cy) },
        {
          x: Math.round(cx + nx * (options.length - 1)),
          y: Math.round(cy + ny * (options.length - 1)),
        },
      ],
      size,
      rgba,
      plot,
    );
    if (tick) dirty = unionRect(dirty, tick);
  }
  return dirty;
}

/**
 * 二次贝塞尔曲线：p0/p2 为端点、p1 为控制点。按控制多边形长度
 * 自适应采样后以笔刷盖章，保证曲线平滑且不断线。
 */
export function strokeQuadCurve(
  doc: RasterDocument,
  p0: { x: number; y: number },
  p1: { x: number; y: number },
  p2: { x: number; y: number },
  size: number,
  rgba: Rgba,
  plot: PlotFn,
): Rect | null {
  const samples = Math.min(
    1024,
    Math.max(16, Math.ceil(Math.hypot(p1.x - p0.x, p1.y - p0.y) + Math.hypot(p2.x - p1.x, p2.y - p1.y))),
  );
  const points: { x: number; y: number }[] = [];
  for (let index = 0; index <= samples; index += 1) {
    const t = index / samples;
    const inv = 1 - t;
    points.push({
      x: Math.round(inv * inv * p0.x + 2 * inv * t * p1.x + t * t * p2.x),
      y: Math.round(inv * inv * p0.y + 2 * inv * t * p1.y + t * t * p2.y),
    });
  }
  return strokePath(doc, points, size, rgba, plot);
}
