import type { RasterDocument } from "@/lib/raster-editor/document";
import { lotRegion } from "@/lib/raster-editor/lotPaint";
import type { LotSlots } from "@/lib/lot-ground/ground";

export function createLotPaintPreview(
  slots: LotSlots,
  widths: readonly number[],
  borders: readonly (readonly number[])[],
  tilePx: readonly [number, number],
  cells: readonly (ImageData | null)[],
) {
  const tiles = cells.map((cell, slot) => {
    const color = slots[slot]?.color;
    if (!cell || !color) return null;
    const tile = document.createElement("canvas");
    tile.width = cell.width;
    tile.height = cell.height;
    const ctx = tile.getContext("2d");
    if (!ctx) return null;
    const tinted = ctx.createImageData(cell.width, cell.height);
    for (let at = 0; at < cell.data.length; at += 4) {
      tinted.data[at] = cell.data[at] * color[0] / 128;
      tinted.data[at + 1] = cell.data[at + 1] * color[1] / 128;
      tinted.data[at + 2] = cell.data[at + 2] * color[2] / 128;
      tinted.data[at + 3] = 255;
    }
    ctx.putImageData(tinted, 0, 0);
    return tile;
  });

  return (ctx: CanvasRenderingContext2D, image: RasterDocument) => {
    const { width, height, pixels } = image;
    const paths = Array.from({ length: 8 }, () => new Path2D());
    for (let y = 0; y < height; y += 1) {
      let previous = -1;
      let start = 0;
      for (let x = 0; x < width; x += 1) {
        const at = (y * width + x) * 4;
        const region = lotRegion(
          [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]],
          widths,
        );
        if (region === previous) continue;
        if (previous >= 0) paths[previous].rect(start, y, x - start, 1);
        previous = region;
        start = x;
      }
      if (previous >= 0) paths[previous].rect(start, y, width - start, 1);
    }

    for (const [region, path] of paths.entries()) {
      const slot = Math.floor(region / 2);
      const main = slots[slot]?.color ?? [128, 128, 128];
      const border = region % 2 === 1;
      const color = border ? borders[slot] ?? main : main;
      const tile = border ? null : tiles[slot];
      const pattern = tile && ctx.createPattern(tile, "repeat");
      if (tile && pattern) {
        pattern.setTransform(new DOMMatrix().scaleSelf(
          tilePx[0] / tile.width,
          tilePx[1] / tile.height,
        ));
      }
      ctx.fillStyle = pattern || `rgb(${color.join(",")})`;
      ctx.fill(path);
    }
  };
}
