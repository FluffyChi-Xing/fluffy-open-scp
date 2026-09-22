<script setup lang="ts">
/**
 * 地图开发面板（v1：区域预览）。
 * 左侧 = 可交互地图预览（滚轮缩放 / 中键拖动 / 米制标尺），
 * 右侧 = 属性与图层区（未来叠加资源视图、地图笔刷等能力）。
 * 渲染管线后端：sc_properties::region_map（341-tile 金字塔 + 全局水位 3336）。
 */
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

interface RegionSummary {
  group: string;
  numericId: string;
  plotCount: number;
}

interface RegionRender {
  pngBase64: string;
  width: number;
  height: number;
  originWorld: [number, number];
  metersPerPixel: number;
  waterPlane: number;
  desert: boolean;
  plotCount: number;
  brushes: [string, [number, number][]][];
}

const packagePath = ref("");
const regions = ref<RegionSummary[]>([]);
const selectedGroup = ref<string>("");
const render = ref<RegionRender | null>(null);
const loading = ref(false);
const errorMsg = ref("");

// 视图状态：缩放（CSS px / 地图像素）与平移
const zoom = ref(1);
const pan = ref({ x: 0, y: 0 });
const panning = ref(false);
let panStart = { x: 0, y: 0, px: 0, py: 0 };
const showPlots = ref(true);
const showResources = ref(true);

const imgUrl = computed(() =>
  render.value ? `data:image/png;base64,${render.value.pngBase64}` : "",
);

async function loadRegions() {
  errorMsg.value = "";
  regions.value = [];
  selectedGroup.value = "";
  render.value = null;
  try {
    loading.value = true;
    regions.value = await invoke<RegionSummary[]>("map_panel_list_regions", {
      packagePath: packagePath.value,
    });
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function renderRegion() {
  if (!selectedGroup.value) return;
  errorMsg.value = "";
  try {
    loading.value = true;
    render.value = await invoke<RegionRender>("map_panel_render_region", {
      packagePath: packagePath.value,
      group: selectedGroup.value,
    });
    zoom.value = 1;
    pan.value = { x: 0, y: 0 };
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loading.value = false;
  }
}

function onWheel(event: WheelEvent) {
  if (!render.value) return;
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const mx = event.clientX - rect.left;
  const my = event.clientY - rect.top;
  const factor = event.deltaY < 0 ? 1.15 : 1 / 1.15;
  const next = Math.min(12, Math.max(0.1, zoom.value * factor));
  // 以鼠标位置为中心缩放
  pan.value.x = mx - ((mx - pan.value.x) * next) / zoom.value;
  pan.value.y = my - ((my - pan.value.y) * next) / zoom.value;
  zoom.value = next;
}

function onMouseDown(event: MouseEvent) {
  if (event.button === 1) {
    panning.value = true;
    panStart = { x: event.clientX, y: event.clientY, px: pan.value.x, py: pan.value.y };
    event.preventDefault();
  }
}
function onMouseMove(event: MouseEvent) {
  if (!panning.value) return;
  pan.value.x = panStart.px + (event.clientX - panStart.x);
  pan.value.y = panStart.py + (event.clientY - panStart.py);
}
function onMouseUp() {
  panning.value = false;
}
</script>

<template>
  <div class="flex h-full min-h-0 gap-2 p-2">
    <!-- 左：地图预览 -->
    <div class="flex min-w-0 flex-1 flex-col gap-2">
      <div class="flex items-center gap-2">
        <input
          v-model="packagePath"
          class="min-w-0 flex-1 rounded border px-2 py-1 text-xs"
          placeholder="区域包路径（例如 …/SimCityData/SimCity_RegionTerrain0.package）"
        />
        <button class="rounded border px-3 py-1 text-xs" @click="loadRegions">
          枚举区域
        </button>
        <select
          v-model="selectedGroup"
          class="rounded border px-2 py-1 text-xs"
          :disabled="!regions.length"
        >
          <option value="" disabled>选择区域</option>
          <option v-for="r in regions" :key="r.group" :value="r.group">
            {{ r.group }} · {{ r.numericId || "?" }} · {{ r.plotCount }} 城
          </option>
        </select>
        <button
          class="rounded border px-3 py-1 text-xs disabled:opacity-40"
          :disabled="!selectedGroup || loading"
          @click="renderRegion"
        >
          {{ loading ? "渲染中（ED 首次较慢）…" : "渲染" }}
        </button>
      </div>
      <div
        v-if="errorMsg"
        class="rounded border border-red-400 bg-red-50 px-2 py-1 text-xs text-red-600 dark:bg-red-900/20"
      >
        {{ errorMsg }}
      </div>
      <!-- 预览区：上/左标尺 + 可缩放画布 -->
      <div class="relative min-h-0 flex-1 overflow-hidden rounded border bg-slate-800">
        <div
          class="absolute inset-0"
          :class="panning ? 'cursor-grabbing' : 'cursor-grab'"
          @wheel="onWheel"
          @mousedown="onMouseDown"
          @mousemove="onMouseMove"
          @mouseup="onMouseUp"
          @mouseleave="onMouseUp"
        >
          <img
            v-if="imgUrl"
            :src="imgUrl"
            draggable="false"
            class="absolute select-none"
            :style="{
              transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
              transformOrigin: '0 0',
              imageRendering: zoom >= 3 ? 'pixelated' : 'auto',
            }"
          />
          <div
            v-else
            class="flex h-full items-center justify-center text-sm text-slate-400"
          >
            加载区域包并渲染后在此预览
          </div>
        </div>
        <!-- 米制比例尺（右上角）：8 m/像素 -->
        <div
          v-if="render"
          class="absolute right-2 top-2 rounded bg-black/50 px-2 py-1 text-[11px] text-white"
        >
          8 m/px · 缩放 {{ (zoom * 100).toFixed(0) }}% · 视野
          {{ Math.round((render.width * render.metersPerPixel) / zoom || 0) }}m
        </div>
      </div>
    </div>

    <!-- 右：面板区（未来地图笔刷 / 资源图层集成处） -->
    <div class="flex w-72 shrink-0 flex-col gap-3 overflow-y-auto rounded border p-3 text-xs">
      <div class="text-sm font-semibold">区域属性</div>
      <template v-if="render">
        <div class="grid grid-cols-2 gap-x-2 gap-y-1">
          <span class="text-neutral-500">尺寸</span><span>{{ render.width }}×{{ render.height }} px</span>
          <span class="text-neutral-500">原点(世界)</span>
          <span>{{ render.originWorld[0].toFixed(0) }}, {{ render.originWorld[1].toFixed(0) }}</span>
          <span class="text-neutral-500">水位面</span><span>{{ render.waterPlane }}</span>
          <span class="text-neutral-500">荒漠模式</span><span>{{ render.desert ? "是" : "否" }}</span>
          <span class="text-neutral-500">城市地块</span><span>{{ render.plotCount }}</span>
          <span class="text-neutral-500">资源画刷</span><span>{{ render.brushes.length }}</span>
        </div>
        <div class="mt-2 text-sm font-semibold">图层</div>
        <label class="flex items-center gap-2">
          <input v-model="showPlots" type="checkbox" /> 城市地块框
        </label>
        <label class="flex items-center gap-2">
          <input v-model="showResources" type="checkbox" /> 资源画刷环
        </label>
        <div class="mt-2 text-neutral-400">
          图层开关与笔刷绘制将在后续版本接入渲染管线（资源多层视图 / 资源绘制）。
        </div>
      </template>
      <div v-else class="text-neutral-400">渲染后显示区域属性与图层。</div>
      <div class="mt-auto text-[11px] text-neutral-400">
        预览管线：341-tile 金字塔拼合 + 全局水位面（3336）+ 湿度×坡度着色。
      </div>
    </div>
  </div>
</template>
