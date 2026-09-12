<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  watch,
} from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import { createDataSource } from "@/api/data-source";
import type {
  DecalDictionaryData,
  DecalEntryMeta,
  DecalImageData,
  Tgi,
} from "@/api/tauri";
import DecalImageViewerSheet from "./DecalImageViewerSheet.vue";

const props = defineProps<{ packageId: number; tgi: Tgi }>();

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
const pending = new Set<number>();
let flushTimer: number | undefined;
let observer: IntersectionObserver | null = null;
const gridRef = ref<HTMLElement | null>(null);

const BATCH_SIZE = 8;

const entries = computed(() => dictionary.value?.entries ?? []);

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

async function load() {
  loading.value = true;
  error.value = null;
  images.clear();
  requested.clear();
  pending.clear();
  observer?.disconnect();
  observer = null;
  try {
    dictionary.value = await source.readDecalDictionary(
      props.packageId,
      props.tgi,
    );
  } catch (cause) {
    dictionary.value = null;
    error.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    loading.value = false;
    await nextTick();
    observeCells();
  }
}

/** 只请求可解码条目；不可解码的单元格直接显示占位与原因。 */
function queue(index: number) {
  const entry = entries.value.find((item) => item.index === index);
  if (!entry?.rasterStatus.decodable) return;
  if (images.has(index) || requested.has(index)) return;
  requested.add(index);
  pending.add(index);
  if (flushTimer !== undefined) window.clearTimeout(flushTimer);
  flushTimer = window.setTimeout(() => void flush(), 50);
}

async function flush() {
  flushTimer = undefined;
  const batch = [...pending].slice(0, BATCH_SIZE);
  if (!batch.length) return;
  for (const index of batch) pending.delete(index);
  try {
    const results = await source.readDecalImages(
      props.packageId,
      props.tgi,
      batch,
    );
    for (const result of results) images.set(result.index, result);
  } catch (cause) {
    const message = cause instanceof Error ? cause.message : String(cause);
    for (const index of batch) {
      images.set(index, {
        index,
        error: message,
        width: null,
        height: null,
        pngBase64: null,
      });
    }
  }
  if (pending.size) flushTimer = window.setTimeout(() => void flush(), 50);
}

function observeCells() {
  observer?.disconnect();
  observer = null;
  const root = gridRef.value;
  if (!root) return;

  if (typeof IntersectionObserver === "undefined") {
    for (const entry of filtered.value) queue(entry.index);
    return;
  }

  observer = new IntersectionObserver(
    (records) => {
      for (const record of records) {
        if (!record.isIntersecting) continue;
        const index = Number((record.target as HTMLElement).dataset.decalIndex);
        if (Number.isFinite(index)) queue(index);
        observer?.unobserve(record.target);
      }
    },
    { root, rootMargin: "200px 0px" },
  );
  root
    .querySelectorAll<HTMLElement>("[data-decal-index]")
    .forEach((cell) => observer?.observe(cell));
}

function openViewer(index: number) {
  viewerIndex.value = index;
  viewerOpen.value = true;
  queue(index);
}

onMounted(() => void load());

onBeforeUnmount(() => {
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
  ],
  () => void load(),
);

watch(filtered, async () => {
  await nextTick();
  observeCells();
});
</script>

<template>
  <div class="decal-gallery">
    <div v-if="dictionary" class="decal-meta">
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

    <p v-if="dictionary && entries.length" class="decal-stats">
      {{
        $t("decal.stats", {
          decodable: stats.decodable,
          missingRaster: stats.missingRaster,
          missingColors: stats.missingColors,
        })
      }}
    </p>

    <div class="decal-search">
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
    <p v-else-if="error" class="decal-empty" role="alert">{{ error }}</p>
    <p v-else-if="!entries.length" class="decal-empty">
      {{ $t("decal.empty") }}
    </p>
    <p v-else-if="!filtered.length" class="decal-empty">
      {{ $t("decal.noResults") }}
    </p>

    <div v-else class="decal-grid-wrap">
      <div ref="gridRef" class="decal-grid" role="list">
        <button
          v-for="entry in filtered"
          :key="entry.index"
          type="button"
          role="listitem"
          class="decal-cell"
          :data-decal-index="entry.index"
          :title="entry.rasterStatus.reason ?? undefined"
          @click="openViewer(entry.index)"
        >
          <span class="decal-cell-thumb">
            <img
              v-if="thumb(entry.index)"
              :src="thumb(entry.index)"
              :alt="hex(entry.id?.instance)"
              loading="lazy"
            />
            <FIcon
              v-else-if="!entry.rasterStatus.decodable"
              name="TriangleAlert"
              :size="16"
              aria-label=""
            />
            <FIcon
              v-else
              name="LoaderCircle"
              :size="16"
              aria-label=""
              class="decal-spin"
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
      @request="queue"
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
.decal-spin {
  animation: decal-spin 900ms linear infinite;
}
@keyframes decal-spin {
  to {
    transform: rotate(360deg);
  }
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
  .decal-spin,
  .decal-skeleton {
    animation: none;
  }
}
</style>
