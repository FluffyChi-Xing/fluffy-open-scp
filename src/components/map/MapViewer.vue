<script setup lang="ts">
/**
 * 地图查看器：缩放 / 平移 / 米制标尺 / HUD（对齐 RasterCanvas 交互模式）。
 * 由地图面板卡片与全屏检查 sheet 共用。
 * 覆盖层：地块框 + 资源画刷环（SVG，随缩放平移同步；图层开关由父级过滤）。
 * L1/L2（priorities P1-4）：pixelated 像素锐利显示、`detail` 窗口高保真
 * 渲染覆盖层、编辑网格、`map-click` 落点与 `viewport-change` 视口事件。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { RegionRender } from "@/lib/region-map";

const props = defineProps<{
  render: RegionRender | null;
  /** 地块框层开关（默认开）。 */
  showPlots?: boolean;
  /** 可见的资源 kind 列表（undefined = 全部显示）。 */
  visibleResources?: string[];
  /** L1 窗口高保真渲染（origin 为窗口左上角世界坐标）。 */
  detail?: RegionRender | null;
  /** 画刷落点模式：十字光标 + map-click 持续上报。 */
  placing?: boolean;
}>();

const emit = defineEmits<{
  /** 左键点击（非拖拽）地图平面，world 为世界坐标米。 */
  (e: "map-click", world: { x: number; y: number }): void;
  /** 缩放/平移/适配后上报视口（worldRect = 可见世界范围，米）。 */
  (
    e: "viewport-change",
    viewport: { zoom: number; worldRect: { x0: number; y0: number; x1: number; y1: number } },
  ): void;
}>();

const { t } = useI18n();

// ── 视图状态（8 m/像素 @ zoom 1）──
const MPP = 8;
/** 32 km 级区域所需的米刻度步长族。 */
const METER_STEPS = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
const wrap = ref<HTMLDivElement | null>(null);
const zoom = ref(1);
const pan = ref({ x: 0, y: 0 });
const panning = ref(false);
const cursorWorld = ref<{ x: number; y: number } | null>(null);
const cursorText = computed(() => {
  if (!cursorWorld.value) return "";
  return `${cursorWorld.value.x}, ${cursorWorld.value.y} m`;
});
let spaceDown = false;
let dragMode: "none" | "pan" = "none";
let panStart = { x: 0, y: 0, px: 0, py: 0 };

const imgUrl = computed(() =>
  props.render ? `data:image/png;base64,${props.render.pngBase64}` : "",
);

// ── 覆盖层（后端已换算为 PNG 像素坐标，前端零换算；显式尺寸防比例失调）──
const planeSize = computed(() =>
  props.render
    ? { width: `${props.render.width}px`, height: `${props.render.height}px` }
    : {},
);

/** 城市地块：2048m = 256px 方框。 */
const plotRects = computed(() => {
  if (!props.render || props.showPlots === false) return [];
  return props.render.plots.map(([x, y]) => ({ x: x - 128, y: y - 128, size: 256 }));
});

/** 可见的资源分布图层。 */
const resourceLayerImgs = computed(() => {
  if (!props.render) return [];
  return props.render.resourceLayers.map((layer) => ({
    ...layer,
    src: `data:image/png;base64,${layer.pngBase64}`,
    visible:
      !props.visibleResources ||
      props.visibleResources.some(
        (k) => k.toLowerCase() === layer.kind.toLowerCase(),
      ),
  }));
});

const xTicks = computed(() => buildTicks(false));
const yTicks = computed(() => buildTicks(true));

function buildTicks(vertical: boolean): { pos: number; label: string }[] {
  const total = (vertical ? props.render?.height : props.render?.width) ?? 0;
  if (!total) return [];
  const step =
    METER_STEPS.find((candidate) => (candidate / MPP) * zoom.value >= 72) ??
    METER_STEPS[METER_STEPS.length - 1];
  const viewSize = vertical
    ? (wrap.value?.clientHeight ?? 600)
    : (wrap.value?.clientWidth ?? 800);
  const ticks: { pos: number; label: string }[] = [];
  for (let meters = 0; meters <= total * MPP + 0.001; meters += step) {
    const pos = pan.value[vertical ? "y" : "x"] + (meters / MPP) * zoom.value;
    if (pos < -48 || pos > viewSize + 48) continue;
    ticks.push({ pos, label: `${Number(meters.toFixed(2))}m` });
  }
  return ticks;
}

function setZoom(value: number) {
  zoom.value = Math.min(32, Math.max(0.05, value));
  emitViewport();
}

/** 可见世界范围（米）——由 viewport 尺寸 + pan/zoom 换算。 */
function visibleWorldRect() {
  const originX = props.render?.originWorld?.[0] ?? -16384;
  const originY = props.render?.originWorld?.[1] ?? -16384;
  const { width, height } = viewportSize.value;
  return {
    x0: originX + (-pan.value.x / zoom.value) * MPP,
    y0: originY + (-pan.value.y / zoom.value) * MPP,
    x1: originX + ((width - pan.value.x) / zoom.value) * MPP,
    y1: originY + ((height - pan.value.y) / zoom.value) * MPP,
  };
}

function emitViewport() {
  if (!props.render) return;
  emit("viewport-change", { zoom: zoom.value, worldRect: visibleWorldRect() });
}

/** L1 detail 覆盖层：以基准图像素坐标定位在 map-plane 内（CSS scale 随缩放生效）。 */
const detailUrl = computed(() =>
  props.detail ? `data:image/png;base64,${props.detail.pngBase64}` : "",
);
const detailStyle = computed(() => {
  const base = props.render;
  const detail = props.detail;
  if (!base || !detail) return null;
  const mpp = base.metersPerPixel || MPP;
  const baseOrigin = base.originWorld ?? [-16384, -16384];
  const detailOrigin = detail.originWorld ?? [-16384, -16384];
  return {
    left: `${(detailOrigin[0] - baseOrigin[0]) / mpp}px`,
    top: `${(detailOrigin[1] - baseOrigin[1]) / mpp}px`,
    width: `${detail.width}px`,
    height: `${detail.height}px`,
  };
});

// ── 编辑网格（L2）：放大后显示世界对齐的方格，屏幕间距自适应 ∈ [24, 48) px ──
const viewportSize = ref({ width: 0, height: 0 });
function updateViewportSize() {
  viewportSize.value = {
    width: wrap.value?.clientWidth ?? 0,
    height: wrap.value?.clientHeight ?? 0,
  };
}
const gridLines = computed(() => {
  if (!props.render || zoom.value < 2) return null;
  const spacingPx = (meters: number) => (meters / MPP) * zoom.value;
  let meters = MPP;
  while (spacingPx(meters) < 24) meters *= 2;
  if (spacingPx(meters) >= 48) return null; // 不应发生（×2 步进），防御
  const originX = props.render.originWorld?.[0] ?? -16384;
  const originY = props.render.originWorld?.[1] ?? -16384;
  const rect = visibleWorldRect();
  const toScreenX = (worldX: number) =>
    ((worldX - originX) / MPP) * zoom.value + pan.value.x;
  const toScreenY = (worldY: number) =>
    ((worldY - originY) / MPP) * zoom.value + pan.value.y;
  const vertical: number[] = [];
  const horizontal: number[] = [];
  for (let x = Math.ceil(rect.x0 / meters) * meters; x <= rect.x1; x += meters) {
    vertical.push(toScreenX(x));
  }
  for (let y = Math.ceil(rect.y0 / meters) * meters; y <= rect.y1; y += meters) {
    horizontal.push(toScreenY(y));
  }
  return { vertical, horizontal, meters };
});

function fit() {
  const element = wrap.value;
  if (!element || !props.render) return;
  const ratio = Math.min(
    (element.clientWidth - 64) / props.render.width,
    (element.clientHeight - 64) / props.render.height,
  );
  setZoom(Math.max(0.05, ratio));
  pan.value = {
    x: (element.clientWidth - props.render.width * zoom.value) / 2,
    y: (element.clientHeight - props.render.height * zoom.value) / 2,
  };
  emitViewport();
}

// ── 指针交互（Pointer 事件 + capture：中键 / 空格平移）──

let downScreen: { x: number; y: number } | null = null;

function onPointerDown(event: PointerEvent) {
  downScreen = { x: event.clientX, y: event.clientY };
  if (event.button === 1 || spaceDown) {
    dragMode = "pan";
    panning.value = true;
    panStart = {
      x: event.clientX,
      y: event.clientY,
      px: pan.value.x,
      py: pan.value.y,
    };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    event.preventDefault();
  }
}

function onPointerMove(event: PointerEvent) {
  const rect = wrap.value?.getBoundingClientRect();
  if (rect) {
    const px = (event.clientX - rect.left - pan.value.x) / zoom.value;
    const py = (event.clientY - rect.top - pan.value.y) / zoom.value;
    const size = props.render?.width ?? 0;
    cursorWorld.value =
      px >= 0 && py >= 0 && px <= size && py <= size
        ? {
            x: Math.round((props.render?.originWorld?.[0] ?? -16384) + px * MPP),
            y: Math.round((props.render?.originWorld?.[1] ?? -16384) + py * MPP),
          }
        : null;
  }
  if (dragMode === "pan") {
    pan.value = {
      x: panStart.px + (event.clientX - panStart.x),
      y: panStart.py + (event.clientY - panStart.y),
    };
  }
}

function onPointerUp(event: PointerEvent) {
  const wasPan = dragMode === "pan";
  dragMode = "none";
  panning.value = false;
  if (wasPan) {
    emitViewport();
    downScreen = null;
    return;
  }
  // 左键点击（移动 ≤4px 视为点击）→ 上报世界落点
  if (downScreen && event.button === 0) {
    const dx = event.clientX - downScreen.x;
    const dy = event.clientY - downScreen.y;
    if (dx * dx + dy * dy <= 16) {
      const rect = wrap.value?.getBoundingClientRect();
      if (rect) {
        const px = (event.clientX - rect.left - pan.value.x) / zoom.value;
        const py = (event.clientY - rect.top - pan.value.y) / zoom.value;
        const size = props.render?.width ?? 0;
        if (px >= 0 && py >= 0 && px <= size && py <= size) {
          emit("map-click", {
            x: Math.round((props.render?.originWorld?.[0] ?? -16384) + px * MPP),
            y: Math.round((props.render?.originWorld?.[1] ?? -16384) + py * MPP),
          });
        }
      }
    }
  }
  downScreen = null;
}

function onWheel(event: WheelEvent) {
  event.preventDefault();
  const next = event.deltaY < 0 ? zoom.value * 1.15 : zoom.value / 1.15;
  const rect = wrap.value?.getBoundingClientRect();
  if (!rect) {
    setZoom(next);
    return;
  }
  // 以指针为缩放锚点：保持指针下的世界坐标在缩放前后不动
  const mx = event.clientX - rect.left;
  const my = event.clientY - rect.top;
  const clamped = Math.min(32, Math.max(0.05, next));
  pan.value = {
    x: mx - ((mx - pan.value.x) / zoom.value) * clamped,
    y: my - ((my - pan.value.y) / zoom.value) * clamped,
  };
  zoom.value = clamped;
  emitViewport();
}

function onKeydown(event: KeyboardEvent) {
  if (event.code === "Space") spaceDown = true;
}
function onKeyup(event: KeyboardEvent) {
  if (event.code === "Space") spaceDown = false;
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("keyup", onKeyup);
  window.addEventListener("resize", updateViewportSize);
  updateViewportSize();
  emitViewport();
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("keyup", onKeyup);
  window.removeEventListener("resize", updateViewportSize);
});

watch(
  () => props.render,
  () => fit(),
);

defineExpose({ fit });
</script>

<template>
  <div
    ref="wrap"
    class="viewer"
    :class="{ panning, placing: props.placing }"
  >
    <div
      class="viewport"
      @wheel="onWheel"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerUp"
    >
      <div
        class="map-plane"
        :style="{
          width: planeSize.width,
          height: planeSize.height,
          transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
        }"
      >
        <img
          v-if="imgUrl"
          :src="imgUrl"
          draggable="false"
          class="map-img pixelated"
          :style="planeSize"
        />
        <!-- L1 窗口高保真渲染：定位在基准图像素坐标系内，随缩放平移同步 -->
        <img
          v-if="detailUrl && detailStyle"
          :src="detailUrl"
          draggable="false"
          class="map-img detail-img"
          :style="detailStyle"
        />
        <img
          v-for="layer in resourceLayerImgs"
          v-show="layer.visible"
          :key="layer.kind"
          :src="layer.src"
          draggable="false"
          class="map-img res-layer"
          :style="planeSize"
          :aria-label="layer.kind"
        />
        <svg
          v-if="render && plotRects.length"
          class="overlay"
          :style="planeSize"
          :viewBox="`0 0 ${render.width} ${render.height}`"
          preserveAspectRatio="none"
        >
          <rect
            v-for="(r, i) in plotRects"
            :key="`p${i}`"
            :x="r.x"
            :y="r.y"
            :width="r.size"
            :height="r.size"
            fill="none"
            stroke="#ffd200"
            stroke-width="3"
          />
        </svg>
      </div>
      <!-- 编辑网格（L2，视口坐标不随 plane 变换，保证 1px 线宽） -->
      <svg v-if="gridLines" class="grid-overlay" aria-hidden="true">
        <line
          v-for="(x, i) in gridLines.vertical"
          :key="`gx${i}`"
          :x1="x"
          y1="0"
          :x2="x"
          :y2="viewportSize.height"
        />
        <line
          v-for="(y, i) in gridLines.horizontal"
          :key="`gy${i}`"
          x1="0"
          :y1="y"
          :x2="viewportSize.width"
          :y2="y"
        />
      </svg>
      <div v-if="!imgUrl" class="empty-hint">
        <FIcon name="Map" :size="28" />
        {{ t("studio.map.emptyPreview") }}
      </div>
    </div>
    <!-- 米制标尺 -->
    <div v-if="render" class="ruler ruler-x" aria-hidden="true">
      <span
        v-for="tick in xTicks"
        :key="`x${tick.label}`"
        class="ruler-tick"
        :style="{ left: `${tick.pos}px` }"
        >{{ tick.label }}</span
      >
    </div>
    <div v-if="render" class="ruler ruler-y" aria-hidden="true">
      <span
        v-for="tick in yTicks"
        :key="`y${tick.label}`"
        class="ruler-tick"
        :style="{ top: `${tick.pos}px` }"
        >{{ tick.label }}</span
      >
    </div>
    <span v-if="render" class="scale-badge">
      1 px = {{ render.metersPerPixel }} m
    </span>
    <!-- HUD -->
    <div v-if="render" class="hud">
      <span v-if="cursorWorld" class="hud-item mono">{{ cursorText }}</span>
      <span class="hud-item">{{ render.width }}×{{ render.height }}</span>
      <span class="hud-item">{{ Math.round(zoom * 100) }}%</span>
      <button
        class="hud-button"
        type="button"
        :title="t('studio.raster.fit')"
        @click="fit"
      >
        <FIcon name="Maximize" :size="12" aria-label="" />
      </button>
      <button
        class="hud-button"
        type="button"
        :title="t('studio.raster.zoom100')"
        @click="setZoom(1)"
      >
        1:1
      </button>
    </div>
  </div>
</template>

<style scoped>
/* 查看器（对齐 RasterCanvas：点阵底 + 标尺 + HUD） */
.viewer {
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
  height: 100%;
  min-height: 0;
  overflow: hidden;
  position: relative;
  width: 100%;
}
.viewer.panning {
  cursor: grab;
}
.viewer.placing {
  cursor: cell;
}
.viewport {
  position: absolute;
  inset: 0;
  overflow: hidden;
}
.map-plane {
  position: absolute;
  top: 0;
  left: 0;
  transform-origin: 0 0;
}
.map-img {
  display: block;
  user-select: none;
  pointer-events: none;
}
.pixelated {
  /* 8m/px 是数据上限：放大显示为锐利色块而非插值糊 */
  image-rendering: pixelated;
}
.detail-img {
  image-rendering: pixelated;
  position: absolute;
}
.grid-overlay {
  inset: 0;
  pointer-events: none;
  position: absolute;
  shape-rendering: crispEdges;
  z-index: 1;
}
.grid-overlay line {
  stroke: color-mix(in srgb, var(--border-strong, var(--border)) 55%, transparent);
  stroke-width: 1px;
}
.res-layer {
  left: 0;
  position: absolute;
  top: 0;
}
.overlay {
  left: 0;
  pointer-events: none;
  position: absolute;
  top: 0;
}
.empty-hint {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.6rem;
  color: var(--subtle-foreground);
  font-size: 0.8125rem;
}
.ruler {
  position: absolute;
  pointer-events: none;
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
  top: 24px;
  z-index: 3;
}
.hud {
  bottom: 8px;
  display: flex;
  gap: 6px;
  position: absolute;
  right: 8px;
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
  padding: 3px 8px;
}
.hud-button {
  cursor: pointer;
}
.hud-button:hover {
  color: var(--foreground);
}
.mono {
  font-family: var(--font-mono, ui-monospace, monospace);
}
</style>
