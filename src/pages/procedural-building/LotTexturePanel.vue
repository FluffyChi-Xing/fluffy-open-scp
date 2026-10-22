<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FInput from "@/components/ui/FInput.vue";
import { tauriApi } from "@/api";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { instanceLabel } from "@/lib/procedural-building/textures";
import { lotLayerPng, lotKey, type LotSelection } from "./lotGround";
import type { Tgi } from "@/api/tauri";

/** Lot 地面遮罩浏览器（PE 物料面板 lots 页签的注入内容）。
 *
 * 【筛选口径 = raster 工作台同款「真实引用」】lot 地面不是按名字圈定
 * 的贴图集，而是「Lot property 经 LotMask key（0x00B1B104 残缺 TGI）
 * 实际引用的 raster」——扫描已打开包的 lot 文档，取 lotMaskResolved
 * （解析命中的完整 TGI + 所在包）去重列出。名字筛不出来，引用筛得出来。
 */
const emit = defineEmits<{
  pick: [payload: LotSelection];
  drop: [payload: LotSelection, clientX: number, clientY: number];
}>();

const store = useGamePackagesStore();

interface MaskEntry {
  /** 遮罩 raster 所在包（跨包解析结果）。 */
  packageId: number;
  tgi: Tgi;
  /** 引用它的 Lot property instance（hex 展示）。 */
  lotLabel: string;
  lotPackageId: number;
  lotTgi: Tgi;
  lotSize: [number, number] | null;
}

const search = ref("");
const entries = ref<MaskEntry[]>([]);
const loading = ref(false);
const loadError = ref(false);
const thumbs = ref(new Map<string, string | null>());
let requestToken = 0;
const pageSize = 32;
const page = ref(0);

const filtered = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  if (!keyword) return entries.value;
  return entries.value.filter(
    (entry) =>
      entry.lotLabel.toLowerCase().includes(keyword) ||
      instanceLabel(entry.tgi).toLowerCase().includes(keyword),
  );
});

const visibleEntries = computed(() =>
  filtered.value.slice(page.value * pageSize, (page.value + 1) * pageSize),
);
watch(filtered, () => {
  page.value = 0;
});

watch(
  () => store.opened.map((p) => p.package.packageId).join(","),
  () => {
    void loadEntries();
  },
);
void loadEntries();

async function loadEntries() {
  const token = ++requestToken;
  loading.value = true;
  loadError.value = false;
  try {
    const collected = new Map<string, MaskEntry>();
    for (const opened of store.opened) {
      if (token !== requestToken) return;
      const documents = await tauriApi.raster.listPropertyDocuments(
        opened.package.packageId,
        "lot",
      );
      if (token !== requestToken) return;
      for (const doc of documents) {
        if (!doc.lotMaskResolved || doc.lotMaskPackageId == null) continue;
        const key = lotKey({
          lotPackageId: opened.package.packageId,
          lotTgi: doc.tgi,
        });
        if (collected.has(key)) continue;
        collected.set(key, {
          packageId: doc.lotMaskPackageId,
          tgi: doc.lotMaskResolved,
          lotLabel: `0x${doc.tgi.instance.toString(16).toUpperCase().padStart(8, "0")}`,
          lotPackageId: opened.package.packageId,
          lotTgi: doc.tgi,
          lotSize: doc.lotSize,
        });
      }
    }
    if (token === requestToken) entries.value = [...collected.values()];
  } catch {
    if (token === requestToken) {
      entries.value = [];
      loadError.value = true;
    }
  } finally {
    if (token === requestToken) loading.value = false;
  }
}

/** Bound work to the current page. Obsolete requests never update thumbnails. */
watch(visibleEntries, async (list, _previous, onCleanup) => {
  let cancelled = false;
  onCleanup(() => {
    cancelled = true;
  });
  for (const item of list) {
    if (cancelled) return;
    const key = lotKey(item);
    if (thumbs.value.has(key)) continue;
    let png: string | null = null;
    try {
      const [source, material] = await Promise.all([
        tauriApi.raster.readSource(item.packageId, item.tgi),
        tauriApi.raster.readLotMaterial(item.lotPackageId, item.lotTgi),
      ]);
      if (cancelled) return;
      png = lotLayerPng(source, material.colors);
    } catch {
      /* Keep an explicit unavailable preview, not a misleading RGB mask. */
    }
    if (cancelled) return;
    const next = new Map(thumbs.value).set(key, png);
    if (next.size > 96) next.delete(next.keys().next().value!);
    thumbs.value = next;
  }
});
function choose(entry: MaskEntry) {
  if (suppressClick) {
    suppressClick = false;
    return;
  }
  emit("pick", {
    packageId: entry.packageId,
    tgi: entry.tgi,
    label: `Lot ${entry.lotLabel}`,
    lotPackageId: entry.lotPackageId,
    lotTgi: entry.lotTgi,
    lotSize: entry.lotSize,
  });
}
let dragEntry: MaskEntry | null = null;
let dragStart = [0, 0];
let suppressClick = false;
function beginDrag(event: PointerEvent, entry: MaskEntry) {
  if (event.button !== 0) return;
  suppressClick = false;
  dragStart = [event.clientX, event.clientY];
  dragEntry = entry;
  window.addEventListener("pointerup", finishDrag, { once: true });
  window.addEventListener("pointercancel", cancelDrag, { once: true });
}
function cancelDrag() {
  dragEntry = null;
  window.removeEventListener("pointerup", finishDrag);
  window.removeEventListener("pointercancel", cancelDrag);
}
function finishDrag(event: PointerEvent) {
  const entry = dragEntry;
  cancelDrag();
  if (!entry) return;
  if (
    Math.hypot(event.clientX - dragStart[0], event.clientY - dragStart[1]) < 5
  )
    return;
  suppressClick = true;
  emit(
    "drop",
    {
      packageId: entry.packageId,
      tgi: entry.tgi,
      label: `Lot ${entry.lotLabel}`,
      lotPackageId: entry.lotPackageId,
      lotTgi: entry.lotTgi,
      lotSize: entry.lotSize,
    },
    event.clientX,
    event.clientY,
  );
}
onBeforeUnmount(() => {
  requestToken++;
  cancelDrag();
});
</script>

<template>
  <div class="lot-panel">
    <FInput
      v-model="search"
      :placeholder="$t('proceduralBuilding.pickerSearch')"
      :aria-label="$t('proceduralBuilding.pickerSearch')"
    />
    <div v-if="loading" class="lot-loading">
      {{ $t("common.loading") }}
    </div>
    <template v-else>
      <p class="lot-hint">{{ $t("proceduralBuilding.lotPickHint") }}</p>
      <div v-if="filtered.length" class="lot-grid">
        <button
          v-for="entry in visibleEntries"
          :key="lotKey(entry)"
          type="button"
          class="lot-entry"
          :title="`${entry.lotLabel} · ${instanceLabel(entry.tgi)}`"
          @click="choose(entry)"
          @pointerdown="beginDrag($event, entry)"
          @dragstart.prevent
        >
          <span class="lot-thumb">
            <img
              v-if="thumbs.get(lotKey(entry))"
              :src="thumbs.get(lotKey(entry))!"
              alt=""
              loading="lazy"
            />
            <FIcon v-else name="Image" :size="18" aria-label="" />
          </span>
          <span class="lot-name">{{ entry.lotLabel }}</span>
          <span class="lot-meta">{{ instanceLabel(entry.tgi) }}</span>
          <span v-if="entry.lotSize" class="lot-size">
            {{
              $t("proceduralBuilding.lotSize", {
                width: entry.lotSize[0].toFixed(1),
                depth: entry.lotSize[1].toFixed(1),
              })
            }}
          </span>
        </button>
      </div>
      <p v-else-if="loadError" class="lot-empty">
        {{ $t("proceduralBuilding.lotScanFailed") }}
      </p>
      <p v-else class="lot-empty">
        {{ $t("proceduralBuilding.lotEmpty") }}
      </p>
      <div v-if="filtered.length > pageSize" class="lot-paging">
        <button type="button" :disabled="page === 0" @click="page--">
          {{ $t("proceduralBuilding.lotPrevious") }}
        </button>
        <span
          >{{ page + 1 }} / {{ Math.ceil(filtered.length / pageSize) }}</span
        >
        <button
          type="button"
          :disabled="(page + 1) * pageSize >= filtered.length"
          @click="page++"
        >
          {{ $t("proceduralBuilding.lotNext") }}
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.lot-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.lot-hint {
  font-size: 12px;
  color: var(--muted-foreground);
  margin: 0;
}
.lot-paging {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.lot-paging button {
  color: inherit;
  background: var(--surface);
  border: 1px solid var(--border);
  padding: 5px;
  border-radius: 4px;
}
.lot-paging button:disabled {
  opacity: 0.4;
}
.lot-loading {
  opacity: 0.65;
  text-align: center;
}
.lot-grid {
  display: grid;
  gap: 10px;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
}
.lot-entry {
  align-items: flex-start;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: inherit;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow: hidden;
  padding: 8px;
  text-align: left;
}
.lot-entry:hover {
  border-color: var(--primary);
}
.lot-thumb {
  align-items: center;
  aspect-ratio: 1 / 1;
  background: repeating-conic-gradient(
      rgba(255, 255, 255, 0.06) 0% 25%,
      transparent 0% 50%
    )
    0 0 / 16px 16px;
  border-radius: 4px;
  display: flex;
  justify-content: center;
  overflow: hidden;
  width: 100%;
}
.lot-thumb img {
  height: 100%;
  object-fit: contain;
  width: 100%;
}
.lot-name {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  width: 100%;
}
.lot-meta {
  font-size: 11px;
  opacity: 0.6;
}
.lot-size {
  font-size: 10px;
  opacity: 0.55;
}
.lot-empty {
  opacity: 0.7;
  text-align: center;
}
</style>
