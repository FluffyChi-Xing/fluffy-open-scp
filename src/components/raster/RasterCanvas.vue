<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import {
  RasterDocument,
  clampRect,
  unionRect,
  type Rect,
} from "@/lib/raster-editor/document";
import { quantizeToImageData, toImageData } from "@/lib/raster-editor/encoders";
import type { RasterHistory } from "@/lib/raster-editor/history";
import {
  drawRect,
  floodFill,
  strokeParkingRow,
  strokePath,
  strokeQuadCurve,
  type Rgba,
} from "@/lib/raster-editor/tools";

/**
 * Raster 绘制画布：原生 canvas 按 1:1 像素持有文档，CSS transform 缩放平移
 * （image-rendering: pixelated 保持像素锐利）。工具交互在组件内闭环，
 * 提交历史后向宿主发 change 事件。
 */
const props = defineProps<{
  doc: RasterDocument;
  history: RasterHistory;
  tool:
    | "brush"
    | "eraser"
    | "line"
    | "rect"
    | "fill"
    | "picker"
    | "parking"
    | "curve";
  color: Rgba;
  brushSize: number;
  /** 油漆桶容差（RGBA 欧氏距离，0 = 精确匹配；抗锯齿图片建议 16-48）。 */
  fillTolerance?: number;
  /** 停车位笔刷：单条停车线长度（px，垂直于拖拽方向）。 */
  parkingLength?: number;
  /** 停车位笔刷：相邻停车线间距（px，沿拖拽方向）。 */
  parkingSpacing?: number;
  /** 量化视图（默认）：阈值化四色清晰结构；关闭则显示原始权重数据。 */
  quantized?: boolean;
  /** 米/像素比例（Lot raster 惯例 0.75）；为 null 时不显示米标尺。 */
  metersPerPixel?: number | null;
}>();
const emit = defineEmits<{
  change: [];
  "pick-color": [rgba: Rgba];
  "zoom-change": [zoom: number];
}>();

const { t } = useI18n();
const canvas = ref<HTMLCanvasElement | null>(null);
const wrap = ref<HTMLDivElement | null>(null);
const zoom = ref(1);
const pan = ref({ x: 0, y: 0 });
const cursor = ref<{ x: number; y: number } | null>(null);
const panning = ref(false);
/** 经 computed 解引用，避免对 props 的直接方法调用（vue/no-mutating-props）。 */
const historyInstance = computed(() => props.history);

let ctx: CanvasRenderingContext2D | null = null;
let spaceDown = false;

/** Lot 比例尺：可选米步长，保证屏幕间距 ≥ 72px。 */
const METER_STEPS = [0.75, 1.5, 3, 6, 12, 24, 48, 96, 192];

const xTicks = computed(() => buildTicks(false));
const yTicks = computed(() => buildTicks(true));

function buildTicks(vertical: boolean): { pos: number; label: string }[] {
  const mpp = props.metersPerPixel;
  if (!mpp) return [];
  const total = (vertical ? props.doc.height : props.doc.width) * mpp;
  const step =
    METER_STEPS.find((candidate) => (candidate / mpp) * zoom.value >= 72) ??
    192;
  const wrapElement = wrap.value;
  const viewSize = vertical
    ? (wrapElement?.clientHeight ?? 600)
    : (wrapElement?.clientWidth ?? 800);
  const ticks: { pos: number; label: string }[] = [];
  for (let meters = 0; meters <= total + 0.001; meters += step) {
    const pos = pan.value[vertical ? "y" : "x"] + (meters / mpp) * zoom.value;
    if (pos < -48 || pos > viewSize + 48) continue;
    ticks.push({ pos, label: `${Number(meters.toFixed(2))}m` });
  }
  return ticks;
}

const checkerboard = computed(() => {
  const size = 8;
  const canvasEl = document.createElement("canvas");
  canvasEl.width = size * 2;
  canvasEl.height = size * 2;
  const context = canvasEl.getContext("2d");
  if (!context) return "";
  context.fillStyle = "#262935";
  context.fillRect(0, 0, size * 2, size * 2);
  context.fillStyle = "#1e202a";
  context.fillRect(0, 0, size, size);
  context.fillRect(size, size, size, size);
  return `url(${canvasEl.toDataURL()})`;
});

function syncCanvasSize() {
  const element = canvas.value;
  if (!element) return;
  element.width = props.doc.width;
  element.height = props.doc.height;
  ctx = element.getContext("2d");
  render();
}

function render() {
  if (!ctx) return;
  const image = props.quantized
    ? quantizeToImageData(props.doc)
    : toImageData(props.doc);
  ctx.putImageData(image, 0, 0);
}

defineExpose({
  render,
  fit: fit,
  undo,
  redo,
  zoomIn: () => setZoom(zoom.value * 1.25),
  zoomOut: () => setZoom(zoom.value / 1.25),
  setZoom: (value: number) => setZoom(value),
});

function setZoom(value: number) {
  zoom.value = Math.min(32, Math.max(0.05, value));
  emit("zoom-change", zoom.value);
}

function fit() {
  const element = wrap.value;
  if (!element) return;
  const available = element.clientWidth - 48;
  const availableHeight = element.clientHeight - 48;
  const ratio = Math.min(
    available / props.doc.width,
    availableHeight / props.doc.height,
    1,
  );
  setZoom(Math.max(0.05, ratio));
  pan.value = { x: 0, y: 0 };
}

// ── 指针交互 ──

type DragMode = "none" | "paint" | "shape" | "pan";
let dragMode: DragMode = "none";
/** 拖拽起点的整幅像素快照：历史 before 与形状预览回滚的共同数据源。 */
let dragBefore: Uint8ClampedArray | null = null;
let dragDirty: Rect | null = null;
let points: { x: number; y: number }[] = [];
let dragStart: { x: number; y: number } | null = null;
let panStart = { x: 0, y: 0, px: 0, py: 0 };
/** 曲线第二阶段：弦长已定，移动鼠标弯曲，单击落笔（Esc 取消）。 */
let curve: {
  p0: { x: number; y: number };
  p2: { x: number; y: number };
  base: Uint8ClampedArray;
} | null = null;

function toPixel(event: PointerEvent): { x: number; y: number } {
  const element = canvas.value!;
  const rect = element.getBoundingClientRect();
  return {
    x: Math.floor((event.clientX - rect.left) / zoom.value),
    y: Math.floor((event.clientY - rect.top) / zoom.value),
  };
}

const cursorText = computed(() => {
  if (!cursor.value) return "";
  const pixel = `${cursor.value.x}, ${cursor.value.y}`;
  if (!props.metersPerPixel) return pixel;
  const mx = ((cursor.value.x + 0.5) * props.metersPerPixel).toFixed(1);
  const my = ((cursor.value.y + 0.5) * props.metersPerPixel).toFixed(1);
  return `${pixel} px · ${mx}, ${my} m`;
});

const eraserColor: Rgba = [0, 0, 0, 0];

function activeColor(): Rgba {
  return props.tool === "eraser" ? eraserColor : props.color;
}

function parkingOptions() {
  return {
    length: Math.max(1, props.parkingLength ?? 13),
    spacing: Math.max(1, props.parkingSpacing ?? 6),
  };
}

/** 形状拖拽的当前帧：先回滚到拖拽前快照再画，文档里永远只有最终形状。 */
function drawShapeDraft(
  from: { x: number; y: number },
  to: { x: number; y: number },
) {
  if (!dragBefore) return;
  props.doc.pixels.set(dragBefore);
  if (props.tool === "line") {
    dragDirty = strokePath(
      props.doc,
      [from, to],
      props.brushSize,
      activeColor(),
      () => {},
    );
  } else if (props.tool === "parking") {
    dragDirty = strokeParkingRow(
      props.doc,
      from,
      to,
      props.brushSize,
      activeColor(),
      parkingOptions(),
      () => {},
    );
  } else if (props.tool === "curve") {
    dragDirty = strokeQuadCurve(
      props.doc,
      from,
      { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 },
      to,
      props.brushSize,
      activeColor(),
      () => {},
    );
  } else {
    dragDirty = drawRect(
      props.doc,
      from,
      to,
      1,
      activeColor(),
      false,
      () => {},
    );
  }
  render();
}

function previewCurveBend(control: { x: number; y: number }) {
  const state = curve;
  if (!state) return;
  props.doc.pixels.set(state.base);
  strokeQuadCurve(
    props.doc,
    state.p0,
    control,
    state.p2,
    props.brushSize,
    activeColor(),
    () => {},
  );
  render();
}

function onPointerDown(event: PointerEvent) {
  if (event.button === 1 || spaceDown) {
    dragMode = "pan";
    panning.value = true;
    panStart = {
      x: event.clientX,
      y: event.clientY,
      px: pan.value.x,
      py: pan.value.y,
    };
    event.preventDefault();
    return;
  }
  if (event.button !== 0) return;
  const pixel = toPixel(event);
  if (props.tool === "picker") {
    if (props.doc.inside(pixel.x, pixel.y)) {
      emit("pick-color", props.doc.getPixel(pixel.x, pixel.y));
    }
    return;
  }
  if (props.tool === "fill") {
    const before = props.doc.pixels.slice();
    const dirty = floodFill(
      props.doc,
      pixel.x,
      pixel.y,
      activeColor(),
      () => {},
      props.fillTolerance ?? 0,
    );
    pushEdit(dirty, before);
    return;
  }
  if (props.tool === "curve" && curve) {
    commitCurve(pixel);
    return;
  }
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  dragBefore = props.doc.pixels.slice();
  dragDirty = null;
  if (
    props.tool === "line" ||
    props.tool === "rect" ||
    props.tool === "parking" ||
    props.tool === "curve"
  ) {
    dragMode = "shape";
    dragStart = pixel;
    return;
  }
  dragMode = "paint";
  points = [pixel];
  dragDirty = strokePath(
    props.doc,
    [pixel],
    props.brushSize,
    activeColor(),
    () => {},
  );
  render();
}

function onPointerMove(event: PointerEvent) {
  const pixel = toPixel(event);
  cursor.value = props.doc.inside(pixel.x, pixel.y) ? pixel : null;
  if (dragMode === "pan") {
    pan.value = {
      x: panStart.px + (event.clientX - panStart.x),
      y: panStart.py + (event.clientY - panStart.y),
    };
    return;
  }
  if (dragMode === "none" && props.tool === "curve" && curve) {
    previewCurveBend(pixel);
    return;
  }
  if (dragMode === "paint") {
    const last = points[points.length - 1];
    if (last && last.x === pixel.x && last.y === pixel.y) return;
    points.push(pixel);
    const dirty = strokePath(
      props.doc,
      points.slice(-2),
      props.brushSize,
      activeColor(),
      () => {},
    );
    if (dirty) {
      dragDirty = unionRect(dragDirty, dirty);
      renderRegion(dirty);
    }
    return;
  }
  if (dragMode === "shape" && dragStart) {
    drawShapeDraft(dragStart, pixel);
  }
}

function onPointerUp(event: PointerEvent) {
  if (dragMode === "pan") {
    dragMode = "none";
    panning.value = false;
    return;
  }
  if (
    dragMode === "shape" &&
    props.tool === "curve" &&
    dragStart &&
    dragBefore
  ) {
    // 弦长确定：不落笔，转入弯曲阶段
    curve = {
      p0: dragStart,
      p2: toPixel(event),
      base: dragBefore,
    };
    dragMode = "none";
    dragStart = null;
    dragBefore = null;
    dragDirty = null;
    props.doc.pixels.set(curve.base);
    render();
    return;
  }
  if (dragMode === "paint" || dragMode === "shape") {
    commitDrag();
  }
}

function onPointerCancel() {
  cancelDraft();
}

function commitDrag() {
  const before = dragBefore;
  pushEdit(dragDirty, before);
  dragMode = "none";
  dragBefore = null;
  dragDirty = null;
  dragStart = null;
  points = [];
}

function commitCurve(control: { x: number; y: number }) {
  const state = curve;
  curve = null;
  if (!state) return;
  props.doc.pixels.set(state.base);
  const dirty = strokeQuadCurve(
    props.doc,
    state.p0,
    control,
    state.p2,
    props.brushSize,
    activeColor(),
    () => {},
  );
  pushEdit(dirty, state.base);
}

/** 统一提交：clamp 脏区 → 从拖拽前快照取 before 入历史栈。 */
function pushEdit(dirty: Rect | null, before: Uint8ClampedArray | null) {
  const rect = dirty ? clampRect(dirty, props.doc) : null;
  if (rect && before) {
    historyInstance.value.push(makeEdit(rect, before));
  }
  render();
  if (rect) emit("change");
}

/** 取消当前草稿（Esc / pointercancel / 切换工具）：回滚文档并清状态。 */
function cancelDraft() {
  if (curve) {
    props.doc.pixels.set(curve.base);
    curve = null;
    render();
    return;
  }
  if (dragBefore) props.doc.pixels.set(dragBefore);
  dragMode = "none";
  dragBefore = null;
  dragDirty = null;
  dragStart = null;
  points = [];
  render();
}

/** 从拖拽前整幅快照截取 before，与当前文档区域 after 组成历史条目。 */
function makeEdit(rect: Rect, beforeFull: Uint8ClampedArray) {
  const after = props.doc.copyRegion(rect);
  const before = new Uint8ClampedArray(rect.w * rect.h * 4);
  for (let row = 0; row < rect.h; row += 1) {
    const srcBase = ((rect.y + row) * props.doc.width + rect.x) * 4;
    before.set(
      beforeFull.subarray(srcBase, srcBase + rect.w * 4),
      row * rect.w * 4,
    );
  }
  return { rect, before, after };
}

function renderRegion(region: Rect) {
  if (!ctx) return;
  const image = props.quantized
    ? quantizeToImageData(props.doc)
    : toImageData(props.doc);
  ctx.putImageData(
    image,
    0,
    0,
    Math.max(0, region.x),
    Math.max(0, region.y),
    Math.min(props.doc.width, region.w),
    Math.min(props.doc.height, region.h),
  );
}

function undo() {
  const edit = historyInstance.value.undo();
  if (edit) {
    render();
    emit("change");
  }
  emitHistoryState();
}

function redo() {
  const edit = historyInstance.value.redo();
  if (edit) {
    render();
    emit("change");
  }
  emitHistoryState();
}

function emitHistoryState() {
  emit("change");
}

// ── 滚轮缩放 / 空格平移 ──

function onWheel(event: WheelEvent) {
  event.preventDefault();
  setZoom(event.deltaY < 0 ? zoom.value * 1.15 : zoom.value / 1.15);
}

function onKeydown(event: KeyboardEvent) {
  if (event.code === "Space") spaceDown = true;
  if (event.code === "Escape") cancelDraft();
}

function onKeyup(event: KeyboardEvent) {
  if (event.code === "Space") spaceDown = false;
}

onMounted(() => {
  syncCanvasSize();
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("keyup", onKeyup);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("keyup", onKeyup);
});

watch(
  () => props.quantized,
  () => render(),
);
watch(
  () => props.tool,
  () => cancelDraft(),
);
watch(
  () => [props.doc.width, props.doc.height],
  () => {
    syncCanvasSize();
    fit();
  },
);
</script>

<template>
  <div
    ref="wrap"
    class="canvas-wrap"
    :class="{ panning }"
    @wheel="onWheel"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerCancel"
  >
    <div
      class="canvas-plane"
      :style="{
        transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
        background: checkerboard,
      }"
    >
      <canvas ref="canvas" class="surface" />
    </div>
    <div v-if="metersPerPixel" class="ruler ruler-x" aria-hidden="true">
      <span
        v-for="tick in xTicks"
        :key="`x${tick.label}`"
        class="ruler-tick"
        :style="{ left: `${tick.pos}px` }"
        >{{ tick.label }}</span
      >
    </div>
    <div v-if="metersPerPixel" class="ruler ruler-y" aria-hidden="true">
      <span
        v-for="tick in yTicks"
        :key="`y${tick.label}`"
        class="ruler-tick"
        :style="{ top: `${tick.pos}px` }"
        >{{ tick.label }}</span
      >
    </div>
    <span v-if="metersPerPixel" class="scale-badge">
      1 px = {{ props.metersPerPixel }} m
    </span>
    <div class="hud">
      <span v-if="cursor" class="hud-item">
        {{ cursorText }}
      </span>
      <span class="hud-item">{{ doc.width }}×{{ doc.height }}</span>
      <span class="hud-item">{{ Math.round(zoom * 100) }}%</span>
      <button
        class="hud-button"
        type="button"
        :title="$t('studio.raster.fit')"
        @click="fit"
      >
        <FIcon name="Maximize" :size="12" aria-label="" />
      </button>
      <button
        class="hud-button"
        type="button"
        :title="$t('studio.raster.zoom100')"
        @click="setZoom(1)"
      >
        1:1
      </button>
    </div>
    <span class="space-hint">{{ t("studio.raster.spacePanHint") }}</span>
  </div>
</template>

<style scoped>
.canvas-wrap {
  background: var(--surface);
  background-image: radial-gradient(
    circle at 1px 1px,
    color-mix(in srgb, var(--border) 60%, transparent) 1px,
    transparent 0
  );
  background-size: 16px 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  cursor: crosshair;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  position: relative;
}
.canvas-wrap.panning {
  cursor: grab;
}
.canvas-plane {
  image-rendering: pixelated;
  position: absolute;
  transform-origin: 0 0;
}
.surface {
  display: block;
  image-rendering: pixelated;
}
.hud {
  bottom: 10px;
  display: flex;
  gap: 6px;
  left: 10px;
  position: absolute;
}
.ruler {
  pointer-events: none;
  position: absolute;
  z-index: 2;
}
.ruler-x {
  background: color-mix(in srgb, var(--surface) 80%, transparent);
  border-bottom: 1px solid var(--border);
  height: 18px;
  left: 0;
  right: 0;
  top: 0;
}
.ruler-y {
  background: color-mix(in srgb, var(--surface) 80%, transparent);
  border-right: 1px solid var(--border);
  bottom: 0;
  left: 0;
  top: 0;
  width: 44px;
}
.ruler-tick {
  color: var(--muted-foreground);
  font-size: 9.5px;
  font-variant-numeric: tabular-nums;
  position: absolute;
  white-space: nowrap;
}
.ruler-x .ruler-tick {
  border-left: 1px solid
    color-mix(in srgb, var(--border-strong, var(--border)) 70%, transparent);
  height: 100%;
  padding: 2px 0 0 3px;
  transform: none;
}
.ruler-y .ruler-tick {
  border-top: 1px solid
    color-mix(in srgb, var(--border-strong, var(--border)) 70%, transparent);
  height: 0;
  padding: 0 2px;
  transform: translateY(-7px);
  width: max-content;
}
.scale-badge {
  background: color-mix(in srgb, var(--surface) 85%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  font-size: 10px;
  padding: 2px 8px;
  position: absolute;
  right: 12px;
  /* 避让顶部 18px 高的 x 轴标尺带并压在其上 */
  top: 24px;
  z-index: 3;
}
.hud-item,
.hud-button {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 85%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
  justify-content: center;
  line-height: 1;
  padding: 0 8px;
  min-height: 20px;
}
.hud-button {
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  font: inherit;
  min-height: 20px;
}
.hud-button:hover {
  color: var(--foreground);
}
.space-hint {
  bottom: 10px;
  color: var(--subtle-foreground);
  font-size: 10px;
  position: absolute;
  right: 12px;
}
</style>
