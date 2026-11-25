import { unionRect, type RasterDocument, type Rect } from "./document";
import { floodFill, type PlotFn, type Rgba } from "./tools";

export interface DecalLayerState {
  visible: boolean;
  locked: boolean;
}

export const defaultDecalLayers: readonly DecalLayerState[] = Array.from(
  { length: 4 }, () => ({ visible: true, locked: false }),
);

export function visibleDecalPixels(
  pixels: Uint8ClampedArray,
  layers: readonly DecalLayerState[] = defaultDecalLayers,
): Uint8ClampedArray {
  const hidden = layers.flatMap(({ visible }, index) => visible ? [] : [index]);
  if (!hidden.length) return pixels;
  const output = pixels.slice();
  for (const channel of hidden)
    for (let offset = channel; offset < output.length; offset += 4) output[offset] = 0;
  return output;
}

export function visibleDecalPixel(pixel: Rgba, layers: readonly DecalLayerState[]): Rgba {
  return [
    layers[0].visible ? pixel[0] : 0,
    layers[1].visible ? pixel[1] : 0,
    layers[2].visible ? pixel[2] : 0,
    layers[3].visible ? pixel[3] : 0,
  ];
}

export function canPaintDecalLayer(layer: DecalLayerState): boolean {
  return layer.visible && !layer.locked;
}

/** Preserve protected channels after a one-hot pixel write. Native composition is A/B/G/R. */
export function protectDecalPaint(
  doc: RasterDocument,
  before: Uint8ClampedArray,
  layers: readonly DecalLayerState[],
  active: number,
  erase = false,
): PlotFn {
  const protectedChannels = layers.flatMap((layer, index) =>
    (erase ? index !== active : !canPaintDecalLayer(layer)) ? [index] : [],
  );
  const below = protectedChannels.filter((index) => index < active && layers[index].locked && layers[index].visible);
  const activeEditable = canPaintDecalLayer(layers[active]);
  return (x, y) => {
    const offset = (y * doc.width + x) * 4;
    if (!activeEditable || (!erase && below.some((index) => before[offset + index] >= 128))) {
      for (let channel = 0; channel < 4; channel++) doc.pixels[offset + channel] = before[offset + channel];
      return;
    }
    for (const channel of protectedChannels) doc.pixels[offset + channel] = before[offset + channel];
  };
}

export function stampDecalLettering(
  doc: RasterDocument,
  before: Uint8ClampedArray,
  glyphs: Uint8ClampedArray,
  layers: readonly DecalLayerState[],
  active: number,
  color: Rgba,
): Rect | null {
  if (!canPaintDecalLayer(layers[active])) return null;
  const protect = protectDecalPaint(doc, before, layers, active);
  let dirty: Rect | null = null;
  for (let offset = 0; offset < glyphs.length; offset += 4) {
    if (glyphs[offset + 3] < 128) continue;
    const x = (offset / 4) % doc.width;
    const y = Math.floor(offset / 4 / doc.width);
    doc.setPixel(x, y, color);
    protect(x, y);
    dirty = unionRect(dirty, { x, y, w: 1, h: 1 });
  }
  return dirty;
}

type Point = { x: number; y: number };

/** Take boundaries from the visible composite at the start of the gesture. */
export function createDecalFillGesture(
  doc: RasterDocument,
  before: Uint8ClampedArray,
  layers: readonly DecalLayerState[],
  active: number,
  color: Rgba,
  tolerance: number,
  throughLocked = false,
) {
  const boundaryLayers = layers.map((layer) => layer.visible && (!throughLocked || !layer.locked));
  const lockedBoundaries = layers.map((layer) => layer.locked && layer.visible && !throughLocked);
  const protect = protectDecalPaint(doc, before, layers, active);
  function boundaryChannel(offset: number) {
    for (let channel = 3; channel >= 0; channel--)
      if (boundaryLayers[channel] && before[offset + channel] >= 128) return channel;
    return -1;
  }
  const workspace = {
    visited: new Uint8Array(doc.width * doc.height),
    pixelAt(x: number, y: number): Rgba {
      const offset = (y * doc.width + x) * 4;
      const channel = boundaryChannel(offset);
      const pixel: [number, number, number, number] = [0, 0, 0, 0];
      if (channel >= 0) pixel[channel] = before[offset + channel];
      return pixel;
    },
    sameRegion(pixel: Rgba, seed: Rgba) {
      return pixel.findIndex((value) => value !== 0) === seed.findIndex((value) => value !== 0);
    },
  };
  let last: Point | null = null;
  return (point: Point): Rect | null => {
    if (!doc.inside(point.x, point.y)) {
      last = null;
      return null;
    }
    const from = last ?? point;
    last = point;
    const dx = point.x - from.x;
    const dy = point.y - from.y;
    const steps = Math.max(1, Math.abs(dx), Math.abs(dy));
    let dirty: Rect | null = null;
    for (let step = 0; step <= steps; step++) {
      const x = Math.round(from.x + dx * step / steps);
      const y = Math.round(from.y + dy * step / steps);
      const top = boundaryChannel((y * doc.width + x) * 4);
      if (top >= 0 && lockedBoundaries[top]) continue;
      const region = floodFill(doc, x, y, color, protect, tolerance, undefined, workspace);
      if (region) dirty = unionRect(dirty, region);
    }
    return dirty;
  };
}
