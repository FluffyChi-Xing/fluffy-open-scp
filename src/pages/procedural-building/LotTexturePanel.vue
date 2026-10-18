<script setup lang="ts">
import { computed, ref, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FInput from "@/components/ui/FInput.vue";
import { tauriApi } from "@/api";
import { useGamePackagesStore } from "@/stores/gamePackages";
import {
  instanceLabel,
  loadRasterPng,
  probeRasterEntry,
} from "@/lib/procedural-building/textures";
import type { Tgi } from "@/api/tauri";

/** Lot 地面遮罩浏览器（PE 物料面板 lots 页签的注入内容）。
 *
 * 【筛选口径 = raster 工作台同款「真实引用」】lot 地面不是按名字圈定
 * 的贴图集，而是「Lot property 经 LotMask key（0x00B1B104 残缺 TGI）
 * 实际引用的 raster」——扫描已打开包的 lot 文档，取 lotMaskResolved
 * （解析命中的完整 TGI + 所在包）去重列出。名字筛不出来，引用筛得出来。
 */
const emit = defineEmits<{
  pick: [payload: { packageId: number; tgi: Tgi; label: string }];
}>();

const store = useGamePackagesStore();

interface MaskEntry {
  /** 遮罩 raster 所在包（跨包解析结果）。 */
  packageId: number;
  tgi: Tgi;
  /** 引用它的 Lot property instance（hex 展示）。 */
  lotLabel: string;
}

const search = ref("");
const entries = ref<MaskEntry[]>([]);
const loading = ref(false);
const loadError = ref(false);
const thumbs = ref(new Map<string, string | null>());
let requestToken = 0;

const filtered = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  if (!keyword) return entries.value;
  return entries.value.filter(
    entry =>
      entry.lotLabel.toLowerCase().includes(keyword) ||
      instanceLabel(entry.tgi).toLowerCase().includes(keyword),
  );
});

watch(
  () => store.opened.map(p => p.package.packageId).join(","),
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
        const key = `${doc.lotMaskPackageId}:${doc.lotMaskResolved.typeId}:${doc.lotMaskResolved.group}:${doc.lotMaskResolved.instance}`;
        if (collected.has(key)) continue;
        collected.set(key, {
          packageId: doc.lotMaskPackageId,
          tgi: doc.lotMaskResolved,
          lotLabel: `0x${doc.tgi.instance.toString(16).toUpperCase().padStart(8, "0")}`,
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

/** 当前过滤结果缩略图懒加载（probe 与宿主共享解码缓存）。 */
watch(filtered, async list => {
  for (const item of list) {
    const key = instanceLabel(item.tgi);
    if (thumbs.value.has(key)) continue;
    const png = await probeRasterEntry(item.packageId, item.tgi);
    const current = thumbs.value;
    current.set(
      key,
      png.decodable
        ? (await loadRasterPng(item.packageId, item.tgi))?.dataUrl ?? null
        : null,
    );
    thumbs.value = new Map(current);
  }
});

function choose(entry: MaskEntry) {
  emit("pick", {
    packageId: entry.packageId,
    tgi: entry.tgi,
    label: `Lot ${entry.lotLabel}`,
  });
}
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
      <div v-if="filtered.length" class="lot-grid">
        <button
          v-for="entry in filtered"
          :key="`${entry.packageId}:${instanceLabel(entry.tgi)}`"
          type="button"
          class="lot-entry"
          :title="`${entry.lotLabel} · ${instanceLabel(entry.tgi)}`"
          @click="choose(entry)"
        >
          <span class="lot-thumb">
            <img
              v-if="thumbs.get(instanceLabel(entry.tgi))"
              :src="thumbs.get(instanceLabel(entry.tgi))!"
              alt=""
              loading="lazy"
            />
            <FIcon v-else name="Image" :size="18" aria-label="" />
          </span>
          <span class="lot-name">{{ entry.lotLabel }}</span>
          <span class="lot-meta">{{ instanceLabel(entry.tgi) }}</span>
        </button>
      </div>
      <p v-else-if="loadError" class="lot-empty">
        {{ $t("proceduralBuilding.lotScanFailed") }}
      </p>
      <p v-else class="lot-empty">
        {{ $t("proceduralBuilding.lotEmpty") }}
      </p>
    </template>
  </div>
</template>

<style scoped>
.lot-panel { display: flex; flex-direction: column; gap: 10px; }
.lot-loading { opacity: 0.65; text-align: center; }
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
.lot-entry:hover { border-color: var(--primary); }
.lot-thumb {
  align-items: center;
  aspect-ratio: 2 / 1;
  background: repeating-conic-gradient(rgba(255, 255, 255, 0.06) 0% 25%, transparent 0% 50%) 0 0 / 16px 16px;
  border-radius: 4px;
  display: flex;
  justify-content: center;
  overflow: hidden;
  width: 100%;
}
.lot-thumb img { height: 100%; object-fit: cover; width: 100%; }
.lot-name { font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; width: 100%; }
.lot-meta { font-size: 11px; opacity: 0.6; }
.lot-empty { opacity: 0.7; text-align: center; }
</style>
