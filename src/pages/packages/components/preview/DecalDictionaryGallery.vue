<script setup lang="ts">
import {
  computed,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  watch,
} from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import { createDataSource } from "@/api/data-source";
import type {
  DecalDictionaryData,
  DecalEntryMeta,
  DecalImageData,
  Tgi,
} from "@/api/tauri";
import DecalImageViewerSheet from "./DecalImageViewerSheet.vue";

const props = defineProps<{
  packageId: number;
  tgi: Tgi;
  compact?: boolean;
  placeable?: boolean;
  scrollRoot?: HTMLElement | null;
}>();
const emit = defineEmits<{ place: [entry: DecalEntryMeta] }>();

const source = createDataSource();
const loading = ref(true);
const error = ref<string | null>(null);
const dictionary = ref<DecalDictionaryData | null>(null);
const search = ref("");
const viewerOpen = ref(false);
const viewerIndex = ref(0);

/** 条目下标 → 已解码图片。 */
const images = reactive(new Map<number, DecalImageData>());
/** 已请求过的下标，避免重复 IPC。 */
const requested = new Set<number>();
const visible = new Set<number>();
let viewerRequest: number | undefined;
let inFlight = false;
let disposed = false;
let flushTimer: number | undefined;
let revision = 0;
let observer: IntersectionObserver | null = null;
const gridRef = ref<HTMLElement | null>(null);

const BATCH_SIZE = 8;

const entries = computed(() => dictionary.value?.entries ?? []);
const entriesByIndex = computed(() => new Map(entries.value.map((entry) => [entry.index, entry])));

const filtered = computed(() => {
  const needle = normalize(search.value);
  if (!needle) return entries.value;
  return entries.value.filter((entry) => searchKeys(entry).includes(needle));
});

const stats = computed(() => {
  let decodable = 0;
  let missingRaster = 0;
  let missingColors = 0;
  for (const entry of entries.value) {
    if (entry.rasterStatus.decodable) decodable += 1;
    if (!entry.rasterStatus.found) missingRaster += 1;
    if (!entry.colorsRgba8) missingColors += 1;
  }
  return { decodable, missingRaster, missingColors };
});

const warnings = computed(() => {
  const list: string[] = [];
  if (dictionary.value && !dictionary.value.uniformArrays) {
    list.push("decal.warnUniform");
  }
  if (dictionary.value?.arrayLengths.some((item) => item.length === null)) {
    list.push("decal.warnMissingArrays");
  }
  return list;
});

function normalize(value: string) {
  return value.trim().toLowerCase().replace(/^0x/, "");
}

function hex8(value: number | null | undefined) {
  return value === null || value === undefined
    ? ""
    : value.toString(16).padStart(8, "0");
}

function hex(value: number | null | undefined) {
  return value === null || value === undefined
    ? "—"
    : `0x${hex8(value).toUpperCase()}`;
}

/** 检索匹配：条目序号、条目 ID、Raster instance 与完整 TGI。 */
function searchKeys(entry: DecalEntryMeta) {
  const keys = [
    `${entry.index}`,
    hex8(entry.id?.instance),
    hex8(entry.raster?.instance),
  ];
  const raster = entry.raster;
  if (raster) {
    keys.push(
      `${hex8(raster.typeId)}:${hex8(raster.group)}:${hex8(raster.instance)}`,
    );
  }
  return keys.filter(Boolean).join(" ");
}

function thumb(index: number) {
  const payload = images.get(index);
  return payload?.pngBase64
    ? `data:image/png;base64,${payload.pngBase64}`
    : undefined;
}

function dims(entry: DecalEntryMeta) {
  const payload = images.get(entry.index);
  const width = payload?.width ?? entry.rasterStatus.width;
  const height = payload?.height ?? entry.rasterStatus.height;
  return width && height ? `${width}×${height}` : "—";
}

function vector(value: [number, number] | null | undefined) {
  return value ? `${value[0]} × ${value[1]}` : "—";
}

function errorMessage(cause: unknown): string {
  if (cause instanceof Error && cause.message) return cause.message;
  if (typeof cause === "string" && cause.trim()) return cause;
  if (cause && typeof cause === "object") {
    const value = cause as Record<string, unknown>;
    for (const key of ["message", "error", "reason"]) {
      if (typeof value[key] === "string" && value[key]) return value[key] as string;
    }
  }
  return "";
}

async function load() {
  const current = ++revision;
  loading.value = true;
  error.value = null;
  images.clear();
  requested.clear();
  visible.clear();
  viewerRequest = undefined;
  if (flushTimer !== undefined) window.clearTimeout(flushTimer);
  flushTimer = undefined;
  observer?.disconnect();
  observer = null;
  try {
    const loaded = await source.readDecalDictionary(
      props.packageId,
      props.tgi,
      props.compact,
    );
    if (current === revision) dictionary.value = loaded;
  } catch (cause) {
    if (current === revision) {
      dictionary.value = null;
      error.value = errorMessage(cause);
    }
  } finally {
    if (current === revision) {
      loading.value = false;
    }
  }
}

function needsImage(index: number) {
  return entriesByIndex.value.get(index)?.rasterStatus.decodable && !requested.has(index);
}

function scheduleFlush() {
  if (disposed || inFlight || flushTimer !== undefined) return;
  if (viewerRequest === undefined && ![...visible].some(needsImage)) return;
  flushTimer = window.setTimeout(() => void flush(), 50);
}

function requestViewerImage(index: number) {
  viewerRequest = needsImage(index) ? index : undefined;
  scheduleFlush();
}

async function flush() {
  const current = revision;
  flushTimer = undefined;
  const indices = [...new Set([viewerRequest, ...visible])]
    .filter((index): index is number => index !== undefined && !!needsImage(index))
    .slice(0, BATCH_SIZE);
  viewerRequest = undefined;
  if (!indices.length) return;
  for (const index of indices) requested.add(index);
  inFlight = true;
  try {
    const results = await source.readDecalImages(
      props.packageId,
      props.tgi,
      indices,
    );
    if (current !== revision) return;
    for (const result of results) images.set(result.index, result);
  } catch (cause) {
    if (current !== revision) return;
    const message = errorMessage(cause) || "decal preview request failed";
    for (const index of indices) {
      images.set(index, {
        index,
        error: message,
        width: null,
        height: null,
        pngBase64: null,
      });
    }
  } finally {
    inFlight = false;
    scheduleFlush();
  }
}

function observeCells() {
  observer?.disconnect();
  observer = null;
  visible.clear();
  const root = gridRef.value;
  if (!root) return;

  if (typeof IntersectionObserver === "undefined") {
    if (!props.compact) {
      for (const entry of filtered.value) visible.add(entry.index);
      scheduleFlush();
    }
    return;
  }

  const nextObserver = new IntersectionObserver(
    (records) => {
      if (observer !== nextObserver) return;
      for (const record of records) {
        const index = Number((record.target as HTMLElement).dataset.decalIndex);
        if (!Number.isFinite(index)) continue;
        if (record.isIntersecting) visible.add(index);
        else visible.delete(index);
      }
      scheduleFlush();
    },
    { root: props.compact ? props.scrollRoot ?? null : root },
  );
  observer = nextObserver;
  root
    .querySelectorAll<HTMLElement>("[data-decal-index]")
    .forEach((cell) => observer?.observe(cell));
}

function openViewer(index: number) {
  viewerIndex.value = index;
  viewerOpen.value = true;
  requestViewerImage(index);
}

function pickForPlacement(event: PointerEvent, entry: DecalEntryMeta): void {
  if (props.placeable && event.button === 0 && entry.id?.instance && entry.rasterStatus.decodable) {
    emit("place", entry);
  }
}

onMounted(() => void load());

onBeforeUnmount(() => {
  disposed = true;
  revision += 1;
  observer?.disconnect();
  observer = null;
  if (flushTimer !== undefined) window.clearTimeout(flushTimer);
});

watch(
  () => [
    props.packageId,
    props.tgi.typeId,
    props.tgi.group,
    props.tgi.instance,
    props.compact,
  ],
  () => void load(),
);

watch([gridRef, filtered, () => props.scrollRoot], observeCells, { flush: "post" });
</script>

<template>
  <div class="decal-gallery" :class="{ 'decal-gallery-compact': compact }">
    <div v-if="dictionary && !compact" class="decal-meta">
      <div class="decal-meta-item">
        <span>{{ $t("decal.material") }}</span>
        <code>{{ hex(dictionary.material?.instance) }}</code>
      </div>
      <div class="decal-meta-item">
        <span>{{ $t("decal.textureSize") }}</span>
        <code>{{ vector(dictionary.textureSize) }}</code>
      </div>
      <div class="decal-meta-item">
        <span>{{ $t("decal.atlasSize") }}</span>
        <code>{{ vector(dictionary.atlasSize) }}</code>
      </div>
      <div class="decal-meta-item">
        <span>{{ $t("decal.entryCount") }}</span>
        <code>{{ entries.length }}</code>
      </div>
    </div>

    <p v-for="key in warnings" :key="key" class="decal-warning" role="status">
      <FIcon name="TriangleAlert" :size="13" aria-label="" />
      {{ $t(key) }}
    </p>

    <p v-if="dictionary && entries.length && !compact" class="decal-stats">
      {{
        $t("decal.stats", {
          decodable: stats.decodable,
          missingRaster: stats.missingRaster,
          missingColors: stats.missingColors,
        })
      }}
    </p>

    <div v-if="!compact" class="decal-search">
      <FIcon name="Search" :size="13" aria-label="" />
      <input
        v-model="search"
        type="search"
        :placeholder="$t('decal.searchPlaceholder')"
        :aria-label="$t('decal.searchPlaceholder')"
      />
      <button
        v-if="search"
        type="button"
        :aria-label="$t('decal.clearSearch')"
        @click="search = ''"
      >
        <FIcon name="X" :size="12" aria-label="" />
      </button>
      <span v-if="search" class="decal-search-count">
        {{ filtered.length }} / {{ entries.length }}
      </span>
    </div>

    <div v-if="loading" class="decal-skeleton-grid" aria-hidden="true">
      <span v-for="slot in 8" :key="slot" class="decal-skeleton" />
    </div>
    <FEmpty
      v-else-if="error"
      variant="compact"
      status="warning"
      icon-name="FileQuestion"
      :title="$t('decal.previewUnavailable')"
      :desc="compact ? undefined : error"
    />
    <FEmpty
      v-else-if="!entries.length"
      variant="compact"
      icon-name="FileText"
      :title="$t('decal.emptyTitle')"
      :desc="$t('decal.empty')"
    />
    <FEmpty v-else-if="!filtered.length" variant="compact" icon-name="SearchX" :title="$t('decal.noResults')" />

    <div v-else class="decal-grid-wrap">
      <div ref="gridRef" class="decal-grid" :role="compact ? undefined : 'list'">
        <button
          v-for="entry in filtered"
          :key="entry.index"
          type="button"
          :role="compact ? undefined : 'listitem'"
          class="decal-cell"
          :data-decal-index="entry.index"
          :class="{ 'decal-cell-placeable': placeable && entry.id?.instance && entry.rasterStatus.decodable }"
          :title="images.get(entry.index)?.error
            ? $t('decal.previewUnavailable')
            : placeable && entry.rasterStatus.decodable
              ? $t('studio.assetWorkbench.decalDragHint')
              : (entry.rasterStatus.reason ?? undefined)"
          @pointerdown="pickForPlacement($event, entry)"
          @click="openViewer(entry.index)"
        >
          <span class="decal-cell-thumb">
            <img
              v-if="thumb(entry.index)"
              :src="thumb(entry.index)"
              :alt="hex(entry.id?.instance)"
              loading="lazy"
              draggable="false"
            />
            <FIcon
              v-else-if="!entry.rasterStatus.decodable || images.get(entry.index)?.error"
              name="TriangleAlert"
              :size="16"
              aria-label=""
            />
            <FIcon
              v-else
              name="Image"
              :size="16"
              aria-label=""
            />
          </span>
          <span class="decal-cell-label">{{ hex(entry.id?.instance) }}</span>
          <span class="decal-cell-dims">{{ dims(entry) }}</span>
          <span v-if="entry.colorsRgba8" class="decal-cell-colors">
            <span
              v-for="(color, slot) in entry.colorsRgba8"
              :key="slot"
              class="decal-swatch"
              :style="{
                background: `rgb(${color[0]} ${color[1]} ${color[2]})`,
              }"
            />
          </span>
        </button>
      </div>
    </div>

    <DecalImageViewerSheet
      v-model:open="viewerOpen"
      :entries="filtered"
      :images="images"
      :start-index="viewerIndex"
      @request="requestViewerImage"
    />
  </div>
</template>

<style scoped>
.decal-gallery {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}
.decal-gallery-compact .decal-grid {
  max-height: none;
  overflow: visible;
}
.decal-grid .decal-cell-placeable {
  cursor: grab;
  touch-action: none;
}
.decal-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 16px;
}
.decal-meta-item {
  align-items: baseline;
  display: flex;
  gap: 6px;
}
.decal-meta-item span {
  color: var(--subtle-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.decal-meta-item code {
  color: var(--foreground);
  font-size: 11px;
}
.decal-warning {
  align-items: center;
  background: color-mix(in srgb, var(--warning, #d97706) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--warning, #d97706) 35%, transparent);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  display: flex;
  font-size: 11px;
  gap: 6px;
  margin: 0;
  padding: 6px 8px;
}
.decal-stats {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 0;
}
.decal-search {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--subtle-foreground);
  display: flex;
  gap: 6px;
  min-height: 30px;
  padding: 0 8px;
}
.decal-search input {
  background: transparent;
  border: 0;
  color: var(--foreground);
  flex: 1;
  font: inherit;
  font-size: 12px;
  min-width: 0;
  outline: none;
}
.decal-search button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--subtle-foreground);
  cursor: pointer;
  display: inline-flex;
  padding: 2px;
}
.decal-search button:hover {
  color: var(--foreground);
}
.decal-search-count {
  color: var(--subtle-foreground);
  font-size: 10px;
}
.decal-grid-wrap {
  container-type: inline-size;
  min-width: 0;
}
.decal-grid {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  max-height: 420px;
  overflow: auto;
  padding: 2px;
}
/* 容器变宽时增加一列，始终保持在 4~5 列。 */
@container (min-width: 560px) {
  .decal-grid {
    grid-template-columns: repeat(5, minmax(0, 1fr));
  }
}
.decal-cell {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 6px 5px;
  text-align: center;
}
.decal-cell:hover {
  background: var(--surface-hover);
  border-color: color-mix(in srgb, var(--border) 60%, var(--foreground));
}
.decal-cell:focus-visible {
  outline: 2px solid var(--accent, #2563eb);
  outline-offset: 1px;
}
.decal-cell-thumb {
  align-items: center;
  display: flex;
  height: 84px;
  justify-content: center;
  width: 100%;
}
.decal-cell-thumb img {
  image-rendering: pixelated;
  max-height: 100%;
  max-width: 100%;
  object-fit: contain;
}
.decal-cell-label {
  color: var(--foreground);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 10px;
}
.decal-cell-dims {
  color: var(--subtle-foreground);
  font-size: 9px;
}
.decal-cell-colors {
  display: inline-flex;
  gap: 2px;
}
.decal-swatch {
  border: 1px solid var(--border);
  border-radius: 2px;
  height: 8px;
  width: 8px;
}
.decal-skeleton-grid {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
}
.decal-skeleton {
  animation: decal-pulse 1.4s ease-in-out infinite;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  height: 118px;
}
@keyframes decal-pulse {
  50% {
    opacity: 0.55;
  }
}
.decal-empty {
  color: var(--subtle-foreground);
  font-size: 12px;
  padding: 26px 12px;
  text-align: center;
}
@media (prefers-reduced-motion: reduce) {
  .decal-skeleton {
    animation: none;
  }
}
</style>
