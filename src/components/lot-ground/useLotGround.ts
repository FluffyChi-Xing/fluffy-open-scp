import { computed, ref, shallowRef, watch, type Ref } from "vue";
import type { Rect, RasterDocument } from "@/lib/raster-editor/document";
import { LOT_PAINT_CHANNELS } from "@/lib/raster-editor/lotPaint";
import type { RasterSchema } from "@/lib/raster-editor/schema";
import type { Rgba } from "@/lib/raster-editor/tools";
import {
  LOT_METERS_PER_PIXEL,
  LOT_SLOT_COUNT,
  clearSlotPixels,
  groundDisplayPixels,
  lotColorsFromSlots,
  lotSizeForMask,
  slotCoverage,
  slotPaint,
  slotsFromLot,
  usedSlots,
  type LotSlots,
} from "@/lib/lot-ground/ground";
import { atlasCellDataUrl, atlasCellImageData, useLotAtlas } from "./lotAtlas";

const DEFAULT_TILE_PERIOD: [number, number] = [8, 8];
const ALL_USED = Array.from({ length: LOT_SLOT_COUNT }, () => true);
const NONE_USED = Array.from({ length: LOT_SLOT_COUNT }, () => false);

export interface LotGroundOptions {
  enabled: Ref<boolean>;
  lot: Ref<RasterSchema["lot"]>;
  doc: Ref<RasterDocument | null>;
  metersPerPixel: Ref<number>;
  changed: () => void;
  applyEdit: (mutate: (pixels: Uint8ClampedArray) => Rect | null) => void;
  confirm: (message: "remove") => boolean;
}

/** Raster 编辑器的 Lot 地面适配层：材质槽 ↔ LotColor、画布显示、底图背景。 */
export function useLotGround(options: LotGroundOptions) {
  const { enabled, lot, doc, metersPerPixel, changed } = options;
  const atlas = useLotAtlas();
  const activeSlot = ref(0);

  const slots = computed<LotSlots>(() =>
    slotsFromLot(lot.value.colors, lot.value.slots ?? NONE_USED),
  );

  // 旧 schema / 读入的游戏 Lot 没有 slots：按遮罩实际覆盖推断已用材质。
  watch(
    () => [enabled.value, doc.value, lot.value.slots] as const,
    ([on, image, used]) => {
      if (!on || !image || used) return;
      const coverage = slotCoverage(image.pixels, lot.value.borderWidths);
      lot.value = { ...lot.value, slots: coverage.map((count) => count > 0) };
    },
    { immediate: true },
  );

  watch(
    slots,
    (value) => {
      if (value[activeSlot.value]) return;
      const first = value.findIndex(Boolean);
      if (first >= 0) activeSlot.value = first;
    },
    { immediate: true },
  );

  const brush = computed<Rgba>(() => slotPaint(activeSlot.value));
  const canvasLayer = computed(() =>
    (LOT_PAINT_CHANNELS as readonly number[]).indexOf(activeSlot.value),
  );

  const patternCells = shallowRef<(ImageData | null)[]>(
    Array.from({ length: LOT_SLOT_COUNT }, () => null),
  );
  let patternRequest = 0;
  watch(
    () =>
      [
        atlas.value,
        slots.value.map((material) => material?.pattern ?? -1).join(","),
      ] as const,
    async ([png]) => {
      const request = ++patternRequest;
      if (!png) {
        patternCells.value = Array.from({ length: LOT_SLOT_COUNT }, () => null);
        return;
      }
      const next = await Promise.all(
        slots.value.map((material) =>
          material ? atlasCellImageData(png, material.pattern) : Promise.resolve(null),
        ),
      );
      if (request === patternRequest) patternCells.value = next;
    },
    { immediate: true },
  );

  const tilePx = computed<[number, number]>(() => {
    const period = lot.value.tilePeriod ?? DEFAULT_TILE_PERIOD;
    const mpp = metersPerPixel.value || LOT_METERS_PER_PIXEL;
    return [period[0] / mpp, period[1] / mpp];
  });

  const display = computed(() => {
    if (!enabled.value) return null;
    const current = slots.value;
    const widths = lot.value.borderWidths ?? [];
    const borders = lot.value.borderColors ?? [];
    const cells = patternCells.value;
    const tile = tilePx.value;
    const image = doc.value;
    return (pixels: Uint8ClampedArray) =>
      groundDisplayPixels(pixels, current, widths, borders, image
        ? { width: image.width, height: image.height, tilePx: tile, cells }
        : undefined);
  });

  const baseCell = ref<string | null>(null);
  let cellRequest = 0;
  watch(
    () => [atlas.value, lot.value.baseTile ?? 8] as const,
    async ([png, tile]) => {
      const request = ++cellRequest;
      const url = png ? await atlasCellDataUrl(png, tile) : null;
      if (request === cellRequest) baseCell.value = url;
    },
    { immediate: true },
  );
  const backdrop = computed(() => {
    if (!enabled.value || !baseCell.value) return null;
    const [tileW, tileH] = tilePx.value;
    return `url(${baseCell.value}) 0 0 / ${tileW}px ${tileH}px repeat`;
  });

  const size = computed<[number, number] | null>(() => {
    if (lot.value.size) return lot.value.size;
    return doc.value ? lotSizeForMask(doc.value.width, doc.value.height) : null;
  });

  function setSlots(next: LotSlots) {
    lot.value = {
      ...lot.value,
      colors: lotColorsFromSlots(next, lot.value.colors),
      slots: usedSlots(next),
    };
    changed();
  }

  function setBaseTile(tile: number) {
    lot.value = { ...lot.value, baseTile: tile };
    changed();
  }

  /** 在没有任何材质时直接落笔：把当前槽登记为材质（沿用其已有 LotColor）。 */
  function ensureActiveUsed() {
    if (slots.value[activeSlot.value]) return;
    const fallback = slotsFromLot(lot.value.colors, ALL_USED)[activeSlot.value];
    setSlots(slots.value.map((material, slot) => (slot === activeSlot.value ? fallback : material)));
  }

  function remove(slot: number) {
    const image = doc.value;
    if (image) {
      let painted = false;
      for (let at = slot; at < image.pixels.length && !painted; at += 4) painted = image.pixels[at] > 0;
      if (painted && !options.confirm("remove")) return;
      if (painted) options.applyEdit((pixels) => clearSlotPixels(pixels, image.width, slot));
    }
    setSlots(slots.value.map((material, index) => (index === slot ? null : material)));
  }

  /** 吸管 / 图层吸取得到的 LOT_PAINT_CHANNELS 下标 → 材质槽。 */
  function pickLayer(layer: number) {
    const slot = LOT_PAINT_CHANNELS[layer];
    if (slot != null) activeSlot.value = slot;
  }

  /** 画布上是否已有该槽像素（换纹理时决定是改当前槽还是新开槽）。 */
  function isSlotPainted(slot: number): boolean {
    const image = doc.value;
    if (!image) return false;
    return slotCoverage(image.pixels, lot.value.borderWidths)[slot] > 0;
  }

  return {
    atlas,
    activeSlot,
    slots,
    brush,
    canvasLayer,
    display,
    backdrop,
    size,
    setSlots,
    setBaseTile,
    ensureActiveUsed,
    remove,
    pickLayer,
    isSlotPainted,
  };
}
