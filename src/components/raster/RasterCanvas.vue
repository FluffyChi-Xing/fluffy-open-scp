<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useResizeObserver } from "@vueuse/core";
import FIcon from "@/components/extensions/FIcon.vue";
import {
  RasterDocument,
  clampRect,
  unionRect,
  type Rect,
} from "@/lib/raster-editor/document";
import { quantizeToImageData, toImageData } from "@/lib/raster-editor/encoders";
import { dataChannelPixels } from "@/lib/raster-editor/profileBehavior";
import type { RasterHistory } from "@/lib/raster-editor/history";
import {
  channelPixels,
  restoreOtherChannels,
  type Channel,
} from "@/lib/raster-editor/schema";
import {
  drawRect,
  floodFill,
  strokeParkingRow,
  strokePath,
  strokeQuadCurve,
  fillPolygon,
  type Rgba,
} from "@/lib/raster-editor/tools";
import type { RasterTool } from "@/lib/raster-editor/schema";
import { drawCircle } from "@/lib/raster-editor/circlePaint";
import { pickedLayer } from "@/lib/raster-editor/profileBehavior";
import { lotRegion } from "@/lib/raster-editor/lotPaint";
import { viewportToPixel } from "@/lib/raster-editor/viewport";
import { oppositeMaskRow } from "@/lib/lot-ground/maskRows";
import {
  canPaintDecalLayer, createDecalFillGesture, protectDecalPaint, visibleDecalPixel,
  type DecalLayerState,
} from "@/lib/raster-editor/decalLayers";

/**
 * 绘制工具使用文档坐标。地面合成预览按屏幕密度绘制材质，通道视图按原始像素显示。
 */
const props = defineProps<{
  doc: RasterDocument;
  history: RasterHistory;
  tool: RasterTool;
  color: Rgba;
  brushSize: number;
  circleRadius?: number;
  /** 油漆桶容差（RGBA 欧氏距离，0 = 精确匹配；抗锯齿图片建议 16-48）。 */
  fillTolerance?: number;
  fillThroughLocked?: boolean;
  /** 停车位笔刷：单条停车线长度（px，垂直于拖拽方向）。 */
  parkingLength?: number;
  /** 停车位笔刷：相邻停车线间距（px，沿拖拽方向）。 */
  parkingSpacing?: number;
  /** 量化视图（默认）：阈值化四色清晰结构；关闭则显示原始权重数据。 */
  quantized?: boolean;
  /** 米/像素比例（Lot raster 惯例 0.75）；为 null 时不显示米标尺。 */
  metersPerPixel?: number | null;
  lotSize?: readonly [number, number] | null;
  channel?: Channel;
  /** Display channel is independent from the channel being painted. */
  displayChannel?: Channel;
  lotLayer?: number | null;
  northUp?: boolean;
  lotBorderWidths?: number[];
  dataChannels?: boolean;
  paintPreview?: ((context: CanvasRenderingContext2D, doc: RasterDocument, region?: Rect) => void) | null;
  /** 画布平面背景（CSS background），透明像素处可见；缺省为棋盘格。 */
  backdrop?: string | null;
  /** 橡皮擦清空全部通道（Lot 地面：擦除 = 露出底图）。 */
  eraseAllChannels?: boolean;
  centerOnResize?: boolean;
  decalLayers?: readonly DecalLayerState[];
  decalActiveLayer?: number;
}>();
const emit = defineEmits<{
  change: [];
  "pick-color": [rgba: Rgba];
  "pick-layer": [layer: number];
  "zoom-change": [zoom: number];
}>();

const { t } = useI18n();
const canvas = ref<HTMLCanvasElement | null>(null);
const wrap = ref<HTMLDivElement | null>(null);
const zoom = ref(1);
const depthScale = computed(() => {
  if (!props.lotSize) return 1;
  const [width, depth] = props.lotSize;
  return (depth / props.doc.height) / (width / props.doc.width);
});
const zoomY = computed(() => zoom.value * depthScale.value);
const metersX = computed(() => props.lotSize
  ? props.lotSize[0] / props.doc.width
  : props.metersPerPixel);
const metersY = computed(() => props.lotSize
  ? props.lotSize[1] / props.doc.height
  : props.metersPerPixel);
const pan = ref({ x: 0, y: 0 });
const cursor = ref<{ x: number; y: number } | null>(null);
const panning = ref(false);
/** 经 computed 解引用，避免对 props 的直接方法调用（vue/no-mutating-props）。 */
const historyInstance = computed(() => props.history);
const documentInstance = computed(() => props.doc);

const viewportSize = ref<{ width: number; height: number }>();
useResizeObserver(
  wrap,
  ([{ target }]) => {
    const width = target.clientWidth;
    const height = target.clientHeight;
    if (!width || !height) return;
    const previous = viewportSize.value;
    if (props.centerOnResize && previous) {
      pan.value = {
        x: pan.value.x + (width - previous.width) / 2,
        y: pan.value.y + (height - previous.height) / 2,
      };
    }
    viewportSize.value = { width, height };
  },
);

let ctx: CanvasRenderingContext2D | null = null;
let spaceDown = false;
let shapeFrame = 0;
let paintFrame = 0;
let pendingPaintRegion: Rect | null = null;
let pendingShapeEnd: { x: number; y: number } | null = null;
function cancelShapeFrame() {
  if (shapeFrame) cancelAnimationFrame(shapeFrame);
  shapeFrame = 0;
  pendingShapeEnd = null;
}

function cancelPaintFrame() {
  if (paintFrame) cancelAnimationFrame(paintFrame);
  paintFrame = 0;
  pendingPaintRegion = null;
}

function queuePaintRegion(region: Rect) {
  if (!lotPreview.value) {
    renderRegion(region);
    return;
  }
  pendingPaintRegion = unionRect(pendingPaintRegion, region);
  if (paintFrame) return;
  paintFrame = requestAnimationFrame(() => {
    const damage = pendingPaintRegion;
    paintFrame = 0;
    pendingPaintRegion = null;
    if (damage) renderRegion(damage);
  });
}

/** Lot 比例尺：可选米步长，保证屏幕间距 ≥ 72px。 */
const METER_STEPS = [0.75, 1.5, 3, 6, 12, 24, 48, 96, 192];

const xTicks = computed(() => buildTicks(false));
const yTicks = computed(() => buildTicks(true));

function buildTicks(vertical: boolean): { pos: number; label: string }[] {
  const mpp = vertical ? metersY.value : metersX.value;
  if (!mpp) return [];
  const axisZoom = vertical ? zoomY.value : zoom.value;
  const total = (vertical ? props.doc.height : props.doc.width) * mpp;
  const step =
    METER_STEPS.find((candidate) => (candidate / mpp) * axisZoom >= 72) ??
    192;
  const viewSize = vertical
    ? (viewportSize.value?.height ?? 600)
    : (viewportSize.value?.width ?? 800);
  const ticks: { pos: number; label: string }[] = [];
  for (let meters = 0; meters <= total + 0.001; meters += step) {
    const pos = pan.value[vertical ? "y" : "x"] + (meters / mpp) * axisZoom;
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

const materialPreview = computed(() =>
  (props.displayChannel ?? "rgba") === "rgba" ? props.paintPreview : null,
);
const lotPreview = computed(() => props.northUp ? materialPreview.value : null);

function render() {
  cancelPaintFrame();
  const element = canvas.value;
  if (!element || !ctx) return;
  const before = dragBefore ?? curve?.base ?? polygonDraft?.base;
  if (before)
    restoreOtherChannels(props.doc.pixels, before, props.channel ?? "rgba");
  const { width, height } = props.doc;
  const preview = materialPreview.value;
  // Lot regions use an integer pixel scale so partial clears have no seams.
  // Pan and zoom can then reuse the same backing store.
  const scale = lotPreview.value
    ? Math.max(1, Math.min(Math.round(window.devicePixelRatio), Math.floor(4096 / Math.max(width, height))))
    : preview
    ? Math.min(zoom.value * window.devicePixelRatio, 4096 / Math.max(width, height))
    : 1;
  const backingWidth = Math.max(1, Math.round(width * scale));
  const backingHeight = Math.max(1, Math.round(height * scale));
  if (element.width !== backingWidth) element.width = backingWidth;
  if (element.height !== backingHeight) element.height = backingHeight;
  if (preview) {
    ctx.setTransform(backingWidth / width, 0, 0, backingHeight / height, 0, 0);
    ctx.clearRect(0, 0, width, height);
    preview(ctx, props.doc);
    return;
  }
  const displayChannel = props.displayChannel ?? "rgba";
  const image =
    displayChannel !== "rgba"
      ? new ImageData(
          new Uint8ClampedArray(channelPixels(props.doc.pixels, displayChannel)),
          props.doc.width,
          props.doc.height,
        )
      : props.quantized
        ? quantizeToImageData(props.doc)
        : props.dataChannels
          ? new ImageData(
              dataChannelPixels(props.doc.pixels),
              props.doc.width,
              props.doc.height,
            )
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
  applyEdit,
  cancelDraft,
});

/** 宿主发起的整幅修改（如移除材质）：同样进入撤销栈。 */
function applyEdit(mutate: (pixels: Uint8ClampedArray, before: Uint8ClampedArray) => Rect | null) {
  cancelDraft();
  const before = props.doc.pixels.slice();
  pushEdit(mutate(props.doc.pixels, before), before);
}

function setZoom(value: number, anchor?: { x: number; y: number }) {
  const next = Math.min(32, Math.max(0.05, value));
  if (next === zoom.value) return;
  if (anchor) {
    const ratio = next / zoom.value;
    pan.value = {
      x: anchor.x - (anchor.x - pan.value.x) * ratio,
      y: anchor.y - (anchor.y - pan.value.y) * ratio,
    };
  }
  zoom.value = next;
  emit("zoom-change", zoom.value);
}

function fit() {
  const element = wrap.value;
  if (!element) return;
  if (element.clientWidth && element.clientHeight)
    viewportSize.value = { width: element.clientWidth, height: element.clientHeight };
  const available = element.clientWidth - 48;
  const availableHeight = element.clientHeight - 48;
  const ratio = Math.min(
    available / props.doc.width,
    availableHeight / (props.doc.height * depthScale.value),
    16,
  );
  setZoom(Math.max(0.05, ratio));
  pan.value = {
    x: (element.clientWidth - props.doc.width * zoom.value) / 2,
    y: (element.clientHeight - props.doc.height * zoomY.value) / 2,
  };
}

// ── 指针交互 ──

type DragMode = "none" | "paint" | "shape" | "pan" | "fill";
let dragMode: DragMode = "none";
let fillGesture: ReturnType<typeof createDecalFillGesture> | null = null;
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
let polygonDraft: { base: Uint8ClampedArray; vertices: { x: number; y: number }[] } | null = null;

function preserveErasedLotLayers(before: Uint8ClampedArray, dirty: Rect | null) {
  if (props.tool !== "eraser" || props.lotLayer == null || props.channel !== "rgba") return;
  if (props.eraseAllChannels) return;
  if (!dirty) return;
  const selected = [3, 0, 1, 2][props.lotLayer];
  const rect = clampRect(dirty, props.doc);
  if (!rect) return;
  for (let y = rect.y; y < rect.y + rect.h; y += 1) {
    for (let x = rect.x; x < rect.x + rect.w; x += 1) {
      const at = (y * props.doc.width + x) * 4;
      for (let channel = 0; channel < 4; channel += 1) {
        if (channel !== selected) documentInstance.value.pixels[at + channel] = before[at + channel];
      }
    }
  }
}

function toPixel(event: PointerEvent): { x: number; y: number } {
  const element = wrap.value!;
  const rect = element.getBoundingClientRect();
  // Wrapper border is outside the plane coordinate system.
  const pixel = viewportToPixel(
    { x: event.clientX, y: event.clientY },
    { left: rect.left + element.clientLeft, top: rect.top + element.clientTop },
    pan.value,
    zoom.value,
    zoomY.value,
  );
  if (props.northUp) pixel.y = oppositeMaskRow(pixel.y, props.doc.height);
  return pixel;
}

function constrainedEnd(from: { x: number; y: number }, to: { x: number; y: number }, shift: boolean) {
  if (!shift) return to;
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  if (props.tool === "rect" || props.tool === "rect-fill") {
    const side = Math.max(Math.abs(dx), Math.abs(dy));
    return { x: from.x + Math.sign(dx || 1) * side, y: from.y + Math.sign(dy || 1) * side };
  }
  if (props.tool === "line" || props.tool === "polyline") {
    const angle = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4);
    const distance = Math.hypot(dx, dy);
    return { x: Math.round(from.x + Math.cos(angle) * distance), y: Math.round(from.y + Math.sin(angle) * distance) };
  }
  return to;
}

const cursorText = computed(() => {
  if (!cursor.value) return "";
  const row = props.northUp ? oppositeMaskRow(cursor.value.y, props.doc.height) : cursor.value.y;
  const pixel = `${cursor.value.x}, ${row}`;
  if (!metersX.value || !metersY.value) return pixel;
  const mx = ((cursor.value.x + 0.5) * metersX.value).toFixed(1);
  const my = ((row + 0.5) * metersY.value).toFixed(1);
  return `${pixel} px · ${mx}, ${my} m`;
});

const eraserColor: Rgba = [0, 0, 0, 0];
const isCircleTool = computed(() => props.tool === "circle" || props.tool === "circle-fill");

function activeColor(): Rgba {
  return props.tool === "eraser" ? eraserColor : props.color;
}

const noPlot = () => {};
function paintPlot(before: Uint8ClampedArray | null) {
  return before && props.decalLayers
    ? protectDecalPaint(props.doc, before, props.decalLayers, props.decalActiveLayer ?? 0, props.tool === "eraser")
    : noPlot;
}

function dragFillAt(pixel: { x: number; y: number }) {
  const dirty = fillGesture?.(pixel);
  if (!dirty) return;
  dragDirty = unionRect(dragDirty, dirty);
  queuePaintRegion(dirty);
}

function parkingOptions() {
  return {
    length: Math.max(1, props.parkingLength ?? 13),
    spacing: Math.max(1, props.parkingSpacing ?? 6),
    mirroredY: props.northUp,
  };
}

function restoreDraftPixels(before: Uint8ClampedArray) {
  const { pixels, width } = props.doc;
  if (!lotPreview.value) {
    pixels.set(before);
    return;
  }
  const rect = dragDirty && clampRect(dragDirty, props.doc);
  if (!rect) return;
  for (let y = rect.y; y < rect.y + rect.h; y++) {
    const offset = (y * width + rect.x) * 4;
    pixels.set(before.subarray(offset, offset + rect.w * 4), offset);
  }
}

/** 形状拖拽的当前帧：先回滚到拖拽前快照再画，文档里永远只有最终形状。 */
function drawShapeDraft(
  from: { x: number; y: number },
  to: { x: number; y: number },
) {
  if (!dragBefore) return;
  const previous = dragDirty;
  restoreDraftPixels(dragBefore);
  const plot = paintPlot(dragBefore);
  if (props.tool === "line") {
    dragDirty = strokePath(
      props.doc,
      [from, to],
      props.brushSize,
      activeColor(),
      plot,
    );
  } else if (props.tool === "parking") {
    dragDirty = strokeParkingRow(
      props.doc,
      from,
      to,
      props.brushSize,
      activeColor(),
      parkingOptions(),
      plot,
    );
  } else if (props.tool === "curve") {
    dragDirty = strokeQuadCurve(
      props.doc,
      from,
      { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 },
      to,
      props.brushSize,
      activeColor(),
      plot,
    );
  } else if (isCircleTool.value) {
    const dragRadius = Math.round(Math.hypot(to.x - from.x, to.y - from.y));
    const radius = dragRadius || props.circleRadius || 16;
    dragDirty = drawCircle(props.doc, from, radius, props.brushSize, activeColor(), props.tool === "circle-fill", plot);
  } else {
    dragDirty = drawRect(
      props.doc,
      from,
      to,
      props.brushSize,
      activeColor(),
      props.tool === "rect-fill",
      plot,
    );
  }
  renderDraftChange(previous);
}

function renderDraftChange(previous: Rect | null) {
  const damage = dragDirty ? unionRect(previous, dragDirty) : previous;
  if (lotPreview.value && damage) renderRegion(damage);
  else if (!lotPreview.value) render();
}

function drawMultiPointDraft(cursor?: { x: number; y: number }) {
  if (!polygonDraft) return;
  const previous = dragDirty;
  restoreDraftPixels(polygonDraft.base);
  const vertices = cursor ? [...polygonDraft.vertices, cursor] : polygonDraft.vertices;
  const plot = paintPlot(polygonDraft.base);
  dragDirty = props.tool === "polygon" && vertices.length >= 3
    ? fillPolygon(props.doc, vertices, activeColor(), plot)
    : strokePath(props.doc, vertices, props.brushSize, activeColor(), plot);
  renderDraftChange(previous);
}

function commitMultiPoint() {
  if (!polygonDraft) return;
  const state = polygonDraft;
  drawMultiPointDraft();
  polygonDraft = null;
  pushEdit(dragDirty, state.base);
  dragDirty = null;
}

function previewCurveBend(control: { x: number; y: number }) {
  const state = curve;
  if (!state) return;
  const previous = dragDirty;
  restoreDraftPixels(state.base);
  dragDirty = strokeQuadCurve(
    props.doc,
    state.p0,
    control,
    state.p2,
    props.brushSize,
    activeColor(),
    paintPlot(state.base),
  );
  renderDraftChange(previous);
}

function onPointerDown(event: PointerEvent) {
  if ((event.target as HTMLElement).closest("button")) return;
  if (event.button === 1 || spaceDown) {
    dragMode = "pan";
    panning.value = true;
    cursor.value = null;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
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
  if (event.target !== canvas.value) return;
  let pixel = toPixel(event);
  if (!props.doc.inside(pixel.x, pixel.y)) return;
  if (props.tool === "layer-picker") {
    if (props.doc.inside(pixel.x, pixel.y)) {
      const selected = pickedLayer("lot-mask", props.doc.getPixel(pixel.x, pixel.y));
      if (selected !== null) emit("pick-layer", selected);
    }
    return;
  }
  if (props.tool === "picker") {
    if (props.doc.inside(pixel.x, pixel.y)) {
      const sample = props.doc.getPixel(pixel.x, pixel.y);
      emit("pick-color", props.decalLayers ? visibleDecalPixel(sample, props.decalLayers) : sample);
    }
    return;
  }
  const activeLayer = props.decalActiveLayer ?? 0;
  if (props.decalLayers && !canPaintDecalLayer(props.decalLayers[activeLayer])) return;
  if (props.tool === "polyline" || props.tool === "polygon") {
    if (!props.doc.inside(pixel.x, pixel.y)) return;
    if (!polygonDraft) polygonDraft = { base: props.doc.pixels.slice(), vertices: [] };
    const vertices = polygonDraft.vertices;
    if (vertices.length && props.tool === "polyline") pixel = constrainedEnd(vertices[vertices.length - 1], pixel, event.shiftKey);
    if (!vertices.length || vertices[vertices.length - 1].x !== pixel.x || vertices[vertices.length - 1].y !== pixel.y)
      vertices.push(pixel);
    drawMultiPointDraft();
    return;
  }
  if (props.tool === "fill") {
    const before = props.doc.pixels.slice();
    if (props.decalLayers) {
      (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
      dragMode = "fill";
      dragBefore = before;
      dragDirty = null;
      fillGesture = createDecalFillGesture(props.doc, before, props.decalLayers, activeLayer, activeColor(), props.fillTolerance ?? 0, props.fillThroughLocked);
      dragFillAt(pixel);
      return;
    }
    const dirty = floodFill(
      props.doc,
      pixel.x,
      pixel.y,
      activeColor(),
      () => {},
      props.fillTolerance ?? 0,
      props.lotLayer != null && (props.channel ?? "rgba") === "rgba"
        ? (pixel) => lotRegion(pixel, props.lotBorderWidths)
        : undefined,
    );
    preserveErasedLotLayers(before, dirty);
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
    props.tool === "rect-fill" ||
    isCircleTool.value ||
    props.tool === "parking" ||
    props.tool === "curve"
  ) {
    dragMode = "shape";
    dragStart = pixel;
    if (isCircleTool.value) drawShapeDraft(pixel, pixel);
    return;
  }
  dragMode = "paint";
  points = [pixel];
  dragDirty = strokePath(
    props.doc,
    [pixel],
    props.brushSize,
    activeColor(),
    paintPlot(dragBefore),
  );
  preserveErasedLotLayers(dragBefore, dragDirty);
  if (lotPreview.value && dragDirty) renderRegion(dragDirty);
  else render();
}

function onPointerMove(event: PointerEvent) {
  if (dragMode === "pan") {
    pan.value = {
      x: panStart.px + (event.clientX - panStart.x),
      y: panStart.py + (event.clientY - panStart.y),
    };
    return;
  }
  const pixel = toPixel(event);
  const nextCursor = props.doc.inside(pixel.x, pixel.y) ? pixel : null;
  if (nextCursor?.x !== cursor.value?.x || nextCursor?.y !== cursor.value?.y)
    cursor.value = nextCursor;
  if (dragMode === "none" && props.tool === "curve" && curve) {
    previewCurveBend(pixel);
    return;
  }
  if (dragMode === "none" && polygonDraft) {
    const last = polygonDraft.vertices[polygonDraft.vertices.length - 1];
    drawMultiPointDraft(last ? constrainedEnd(last, pixel, event.shiftKey) : pixel);
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
      paintPlot(dragBefore),
    );
    if (dirty) {
      if (dragBefore) preserveErasedLotLayers(dragBefore, dirty);
      dragDirty = unionRect(dragDirty, dirty);
      queuePaintRegion(dirty);
    }
    return;
  }
  if (dragMode === "fill") {
    dragFillAt(pixel);
    return;
  }
  if (dragMode === "shape" && dragStart) {
    pendingShapeEnd = constrainedEnd(dragStart, pixel, event.shiftKey);
    if (!shapeFrame) shapeFrame = requestAnimationFrame(() => {
      shapeFrame = 0;
      if (dragMode === "shape" && dragStart && pendingShapeEnd)
        drawShapeDraft(dragStart, pendingShapeEnd);
      pendingShapeEnd = null;
    });
  }
}

function onPointerLeave() {
  if (dragMode === "none") cursor.value = null;
}

function onPointerUp(event: PointerEvent) {
  cancelShapeFrame();
  const target = event.currentTarget as HTMLElement;
  if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
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
    const previous = dragDirty;
    restoreDraftPixels(curve.base);
    dragDirty = null;
    renderDraftChange(previous);
    return;
  }
  if (dragMode === "paint" || dragMode === "shape" || dragMode === "fill") {
    if (dragMode === "fill") dragFillAt(toPixel(event));
    if (dragMode === "shape" && dragStart && props.tool !== "curve")
      drawShapeDraft(dragStart, constrainedEnd(dragStart, toPixel(event), event.shiftKey));
    commitDrag();
  }
}

function onDoubleClick() {
  if (polygonDraft) commitMultiPoint();
}

function onPointerCancel(event: PointerEvent) {
  const target = event.currentTarget as HTMLElement;
  if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
  panning.value = false;
  cancelDraft();
}

function commitDrag() {
  cancelShapeFrame();
  const before = dragBefore;
  if (before) preserveErasedLotLayers(before, dragDirty);
  pushEdit(dragDirty, before);
  dragMode = "none";
  fillGesture = null;
  dragBefore = null;
  dragDirty = null;
  dragStart = null;
  points = [];
}

function commitCurve(control: { x: number; y: number }) {
  const state = curve;
  curve = null;
  if (!state) return;
  restoreDraftPixels(state.base);
  const dirty = strokeQuadCurve(
    props.doc,
    state.p0,
    control,
    state.p2,
    props.brushSize,
    activeColor(),
    paintPlot(state.base),
  );
  pushEdit(dirty, state.base, dragDirty);
  dragDirty = null;
}

/** 统一提交：clamp 脏区 → 从拖拽前快照取 before 入历史栈。 */
function pushEdit(dirty: Rect | null, before: Uint8ClampedArray | null, previous: Rect | null = null) {
  const rect = dirty ? clampRect(dirty, props.doc) : null;
  let didChange = !!rect;
  if (rect && before) {
    restoreOtherChannels(props.doc.pixels, before, props.channel ?? "rgba");
    const edit = makeEdit(rect, before);
    didChange = !props.decalLayers || edit.after.some((value, index) => value !== edit.before[index]);
    if (didChange) historyInstance.value.push(edit);
  }
  if (lotPreview.value) {
    const damage = rect ? unionRect(previous, rect) : previous;
    if (damage) queuePaintRegion(damage);
  } else render();
  if (didChange) emit("change");
}

/** 取消当前草稿（Esc / pointercancel / 切换工具）：回滚文档并清状态。 */
function cancelDraft() {
  cancelShapeFrame();
  if (polygonDraft) {
    const previous = dragDirty;
    restoreDraftPixels(polygonDraft.base);
    polygonDraft = null;
    dragDirty = null;
    renderDraftChange(previous);
    return;
  }
  if (curve) {
    const previous = dragDirty;
    restoreDraftPixels(curve.base);
    curve = null;
    dragDirty = null;
    renderDraftChange(previous);
    return;
  }
  const previous = dragBefore ? dragDirty : null;
  if (dragBefore) restoreDraftPixels(dragBefore);
  dragMode = "none";
  panning.value = false;
  fillGesture = null;
  dragBefore = null;
  dragDirty = null;
  dragStart = null;
  points = [];
  renderDraftChange(previous);
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
  const rect = clampRect(region, props.doc);
  if (!rect) return;
  const preview = lotPreview.value;
  if (preview) {
    const before = dragBefore ?? curve?.base ?? polygonDraft?.base;
    if (before) restoreOtherChannels(props.doc.pixels, before, props.channel ?? "rgba");
    ctx.clearRect(rect.x, rect.y, rect.w, rect.h);
    preview(ctx, props.doc, rect);
    return;
  }
  if (materialPreview.value) {
    render();
    return;
  }
  const pixels = props.doc.copyRegion(rect);
  const displayChannel = props.displayChannel ?? "rgba";
  const image = displayChannel !== "rgba"
    ? new ImageData(new Uint8ClampedArray(channelPixels(pixels, displayChannel)), rect.w, rect.h)
    : props.quantized
      ? quantizeToImageData(new RasterDocument(rect.w, rect.h, pixels))
      : new ImageData(new Uint8ClampedArray(props.dataChannels ? dataChannelPixels(pixels) : pixels), rect.w, rect.h);
  ctx.putImageData(image, rect.x, rect.y);
}

function undo() {
  const edit = historyInstance.value.undo();
  if (edit) {
    if (lotPreview.value) queuePaintRegion(edit.rect);
    else render();
    emit("change");
  }
  emitHistoryState();
}

function redo() {
  const edit = historyInstance.value.redo();
  if (edit) {
    if (lotPreview.value) queuePaintRegion(edit.rect);
    else render();
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
  if (!event.deltaY) return;
  const element = props.northUp ? wrap.value : null;
  const rect = element?.getBoundingClientRect();
  const anchor = element && rect ? {
    x: event.clientX - rect.left - element.clientLeft,
    y: event.clientY - rect.top - element.clientTop,
  } : undefined;
  setZoom(event.deltaY < 0 ? zoom.value * 1.15 : zoom.value / 1.15, anchor);
}

function onKeydown(event: KeyboardEvent) {
  if (event.code === "Space") spaceDown = true;
  if (event.code === "Enter" && polygonDraft) {
    event.preventDefault();
    commitMultiPoint();
  }
  if (event.code === "Escape") cancelDraft();
}

function onKeyup(event: KeyboardEvent) {
  if (event.code === "Space") spaceDown = false;
}

onMounted(() => {
  ctx = canvas.value?.getContext("2d") ?? null;
  fit();
  render();
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("keyup", onKeyup);
});
onBeforeUnmount(() => {
  cancelShapeFrame();
  cancelPaintFrame();
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("keyup", onKeyup);
});

watch(
  () => [props.quantized, props.displayChannel, props.dataChannels],
  () => {
    cancelDraft();
    render();
  },
);
watch(
  () => [materialPreview.value, materialPreview.value && !props.northUp ? zoom.value : 1],
  () => render(),
);
watch(
  () => props.tool,
  () => cancelDraft(),
);
watch(() => [props.decalLayers, props.decalActiveLayer], () => cancelDraft());
watch(
  () => props.doc,
  () => {
    cancelShapeFrame();
    dragBefore = null;
    dragDirty = null;
    curve = null;
    polygonDraft = null;
    dragMode = "none";
    fillGesture = null;
    points = [];
    fit();
    render();
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
    @pointerleave="onPointerLeave"
    @pointerup="onPointerUp"
    @pointercancel="onPointerCancel"
    @dblclick="onDoubleClick"
  >
    <div
      class="canvas-plane"
      :class="{ 'lot-plane': northUp }"
      :style="{
        transform: northUp
          ? `translate(${pan.x}px, ${pan.y + doc.height * zoomY}px) scale(${zoom}, ${-zoomY})`
          : `translate(${pan.x}px, ${pan.y}px) scale(${zoom}, ${zoomY})`,
        background: backdrop || checkerboard,
      }"
    >
      <canvas
        ref="canvas"
        class="surface"
        :class="{ 'material-preview': materialPreview }"
        :style="{ width: `${doc.width}px`, height: `${doc.height}px` }"
      />
      <slot />
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
      1 px ≈ {{ Number(metersX?.toPrecision(4)) }} m
    </span>
    <div class="hud" :class="{ 'with-ruler': metersPerPixel }">
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
  cursor: default;
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
.lot-plane { will-change: transform; }
.surface {
  display: block;
  image-rendering: pixelated;
  cursor: crosshair;
}
.surface.material-preview {
  image-rendering: auto;
}
.hud {
  z-index: 3;
  bottom: 10px;
  display: flex;
  gap: 6px;
  left: 10px;
  position: absolute;
}
.hud.with-ruler {
  left: 54px;
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
@media (max-width: 1100px) {
  .space-hint {
    display: none;
  }
}
</style>
