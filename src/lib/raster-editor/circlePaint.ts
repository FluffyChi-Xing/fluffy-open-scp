import type { RasterDocument, Rect } from "./document";
import type { PlotFn, Rgba } from "./tools";

/** Rasterize around a pixel center. The outline stays inside the fill boundary. */
export function drawCircle(
  doc: RasterDocument,
  center: { x: number; y: number },
  radius: number,
  strokeWidth: number,
  color: Rgba,
  filled: boolean,
  plot: PlotFn,
): Rect | null {
  const { x: centerX, y: centerY } = center;
  const outerRadius2 = radius ** 2;
  const innerRadius2 = filled ? 0 : Math.max(0, radius - Math.max(1, strokeWidth)) ** 2;
  let minX = doc.width;
  let minY = doc.height;
  let maxX = -1;
  let maxY = -1;
  for (let y = Math.max(0, centerY - radius); y <= Math.min(doc.height - 1, centerY + radius); y++) {
    const y2 = (y - centerY) ** 2;
    for (let x = Math.max(0, centerX - radius); x <= Math.min(doc.width - 1, centerX + radius); x++) {
      const distance2 = (x - centerX) ** 2 + y2;
      if (distance2 > outerRadius2 || distance2 < innerRadius2) continue;
      doc.setPixel(x, y, color);
      plot(x, y);
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
    }
  }
  return maxX < minX ? null : { x: minX, y: minY, w: maxX - minX + 1, h: maxY - minY + 1 };
}
