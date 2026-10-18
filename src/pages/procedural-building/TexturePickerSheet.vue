<script setup lang="ts">
import { computed, ref, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FButton from "@/components/ui/FButton.vue";
import FInput from "@/components/ui/FInput.vue";
import FSelect from "@/components/ui/FSelect.vue";
import FSheet from "@/components/ui/FSheet.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { useGamePackagesStore } from "@/stores/gamePackages";
import {
  type RasterCatalogEntry,
  instanceLabel,
  listRasterEntries,
  loadRasterPng,
  probeRasterEntry,
} from "@/lib/procedural-building/textures";
import type { Tgi } from "@/api/tauri";

/** 游戏 raster 贴图浏览器：从已打开的 package 中挑一张贴图绑定到
 * 程序化建筑的材质槽。分页浏览 + 当前页懒加载缩略图；不可解码
 * （DXT 压缩）条目置灰不可选。 */
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{
  pick: [payload: { packageId: number; tgi: Tgi; label: string }];
}>();

const store = useGamePackagesStore();

const packageOptions = computed(() =>
  store.opened.map(entry => ({
    label: entry.package.path.split(/[\\/]/).pop() ?? entry.package.path,
    value: String(entry.package.packageId),
  })),
);
const selectedPackage = ref("");
watch(
  packageOptions,
  options => {
    if (!options.length) return;
    if (!options.some(option => option.value === selectedPackage.value)) {
      selectedPackage.value = options[0].value;
    }
  },
  { immediate: true },
);

const PAGE_SIZE = 48;
const search = ref("");
const entries = ref<RasterCatalogEntry[]>([]);
const loading = ref(false);
const page = ref(0);
const thumbs = ref(new Map<string, string | null>());
let requestToken = 0;

const totalPages = computed(() => Math.max(1, Math.ceil(entries.value.length / PAGE_SIZE)));
const pageEntries = computed(() =>
  entries.value.slice(page.value * PAGE_SIZE, (page.value + 1) * PAGE_SIZE),
);

watch([selectedPackage, search], () => {
  page.value = 0;
  void loadEntries();
});
watch(open, value => {
  if (value) void loadEntries();
});

async function loadEntries() {
  const packageId = Number(selectedPackage.value);
  if (!packageId) {
    entries.value = [];
    return;
  }
  const token = ++requestToken;
  loading.value = true;
  try {
    const list = await listRasterEntries(packageId, search.value);
    if (token !== requestToken) return;
    entries.value = list;
  } catch {
    if (token === requestToken) entries.value = [];
  } finally {
    if (token === requestToken) loading.value = false;
  }
}

/** 当前页逐条探测（可解码性/尺寸 + 缩略图；probe 与 pick 共享解码缓存）。 */
watch(pageEntries, async list => {
  const packageId = Number(selectedPackage.value);
  if (!packageId) return;
  for (const item of list) {
    const key = instanceLabel(item.tgi);
    if (thumbs.value.has(key)) continue;
    const probe = await probeRasterEntry(packageId, item.tgi);
    item.decodable = probe.decodable;
    item.width = probe.width;
    item.height = probe.height;
    const current = thumbs.value;
    current.set(
      key,
      probe.decodable
        ? (await loadRasterPng(packageId, item.tgi))?.dataUrl ?? null
        : null,
    );
    thumbs.value = new Map(current);
  }
});

function choose(entry: RasterCatalogEntry) {
  if (!entry.decodable) return;
  emit("pick", {
    packageId: entry.packageId,
    tgi: entry.tgi,
    label: entry.label,
  });
  open.value = false;
}
</script>

<template>
  <FSheet v-model:open="open" :label="$t('proceduralBuilding.pickerTitle')" width="min(720px, 94vw)">
    <div class="picker-body">
      <div class="picker-controls">
        <FSelect
          v-model="selectedPackage"
          :options="packageOptions"
          :aria-label="$t('proceduralBuilding.pickerPackage')"
        />
        <FInput
          v-model="search"
          :placeholder="$t('proceduralBuilding.pickerSearch')"
          :aria-label="$t('proceduralBuilding.pickerSearch')"
        />
      </div>
      <div v-if="loading" class="picker-loading">
        <FSpinner size="sm" :label="$t('common.loading')" />
      </div>
      <template v-else>
        <div v-if="pageEntries.length" class="picker-grid">
          <button
            v-for="entry in pageEntries"
            :key="instanceLabel(entry.tgi)"
            type="button"
            class="picker-entry"
            :class="{ disabled: !entry.decodable }"
            :disabled="!entry.decodable"
            :title="entry.decodable ? entry.label : $t('proceduralBuilding.pickerNotDecodable')"
            @click="choose(entry)"
          >
            <span class="picker-thumb">
              <img
                v-if="thumbs.get(instanceLabel(entry.tgi))"
                :src="thumbs.get(instanceLabel(entry.tgi))!"
                alt=""
                loading="lazy"
              />
              <FIcon v-else name="Image" :size="18" aria-label="" />
            </span>
            <span class="picker-name">{{ entry.label }}</span>
            <span class="picker-meta">
              {{
                entry.decodable
                  ? $t("proceduralBuilding.pickerSize", {
                      width: entry.width ?? "?",
                      height: entry.height ?? "?",
                    })
                  : instanceLabel(entry.tgi)
              }}
            </span>
          </button>
        </div>
        <p v-else class="picker-empty">{{ $t("proceduralBuilding.pickerEmpty") }}</p>
        <div v-if="totalPages > 1" class="picker-pager">
          <FButton :disabled="page <= 0" @click="page -= 1">
            <FIcon name="ChevronLeft" :size="14" aria-label="" />
          </FButton>
          <span>{{ page + 1 }} / {{ totalPages }}</span>
          <FButton :disabled="page >= totalPages - 1" @click="page += 1">
            <FIcon name="ChevronRight" :size="14" aria-label="" />
          </FButton>
        </div>
      </template>
    </div>
  </FSheet>
</template>

<style scoped>
.picker-body { display: flex; flex-direction: column; gap: 12px; padding: 16px; }
.picker-controls { display: grid; gap: 8px; grid-template-columns: minmax(180px, 1fr) 2fr; }
.picker-loading { align-items: center; display: flex; justify-content: center; min-height: 200px; }
.picker-grid { display: grid; gap: 10px; grid-template-columns: repeat(auto-fill, minmax(128px, 1fr)); }
.picker-entry {
  align-items: flex-start;
  background: var(--bg-soft, rgba(255, 255, 255, 0.04));
  border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  border-radius: var(--radius-md, 8px);
  color: inherit;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow: hidden;
  padding: 8px;
  text-align: left;
}
.picker-entry:hover:not(.disabled) { border-color: var(--primary, #4c8dff); }
.picker-entry.disabled { cursor: not-allowed; opacity: 0.45; }
.picker-thumb {
  align-items: center;
  aspect-ratio: 1;
  background: repeating-conic-gradient(rgba(255, 255, 255, 0.06) 0% 25%, transparent 0% 50%) 0 0 / 16px 16px;
  border-radius: 4px;
  display: flex;
  justify-content: center;
  overflow: hidden;
  width: 100%;
}
.picker-thumb img { height: 100%; object-fit: contain; width: 100%; }
.picker-name { font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; width: 100%; }
.picker-meta { font-size: 11px; opacity: 0.6; }
.picker-empty { opacity: 0.7; text-align: center; }
.picker-pager { align-items: center; display: flex; gap: 12px; justify-content: center; }
</style>
