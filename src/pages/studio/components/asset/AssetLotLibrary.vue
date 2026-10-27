<script setup lang="ts">
import { computed, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { tauriApi } from "@/api";
import type { PropertyDocumentSummary, Tgi } from "@/api/tauri";
import { useGamePackagesStore } from "@/stores/gamePackages";
import AssetLotDiagram from "./AssetLotDiagram.vue";

/** 应用载荷：Lot 资源位置（会话层据此读取并套用尺寸/四色分层）。 */
export interface LotPresetRef {
  packageId: number;
  tgi: Tgi;
}

/**
 * 右栏 "Lot 库"：扫描全部已打开包的 Lot property（0x00B1B104），按所属包
 * 分组列出，每条渲染 raster 面板同款 Lot 分层示意图（ground-only 视口）。
 * 草稿模式下可一键把该 Lot 的尺寸与四色分层应用到当前地块。
 */
const props = defineProps<{
  editEnabled: boolean;
  editMode: "native" | "draft";
}>();
const emit = defineEmits<{ apply: [preset: LotPresetRef] }>();

const { t } = useI18n();
const packages = useGamePackagesStore();
const busy = ref(false);
const error = ref("");
const search = ref("");
const collapsed = ref(new Set<string>());

interface LotEntry {
  packageId: number;
  packageName: string;
  tgi: Tgi;
  lotSize: [number, number] | null;
}
const groups = shallowRef<{ name: string; entries: LotEntry[] }[]>([]);
const PER_GROUP_CAP = 48;

const packageNames = computed(() => {
  const names = new Map<number, string>();
  for (const opened of packages.opened)
    names.set(
      opened.package.packageId,
      opened.package.path.split(/[\\/]/).pop() ?? opened.package.path,
    );
  return names;
});

async function scan() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    const results = await Promise.all(
      packages.opened.map(async (opened) => {
        const packageId = opened.package.packageId;
        const packageName =
          opened.package.path.split(/[\\/]/).pop() ?? opened.package.path;
        try {
          const documents = await tauriApi.raster.listPropertyDocuments(
            packageId,
            "lot",
          );
          return documents.map(
            (document: PropertyDocumentSummary): LotEntry => ({
              packageId,
              packageName,
              tgi: document.tgi,
              lotSize: document.lotSize ?? null,
            }),
          );
        } catch {
          return [] as LotEntry[];
        }
      }),
    );
    groups.value = results
      .flat()
      .reduce<{ name: string; entries: LotEntry[] }[]>((acc, entry) => {
        const group = acc.find((item) => item.name === entry.packageName);
        if (group) group.entries.push(entry);
        else acc.push({ name: entry.packageName, entries: [entry] });
        return acc;
      }, [])
      .sort((a, b) => a.name.localeCompare(b.name));
  } catch (cause) {
    error.value = String(cause);
  } finally {
    busy.value = false;
  }
}
onMounted(scan);
watch(
  () => packages.opened.map((p) => p.package.packageId).join(","),
  () => void scan(),
);

const visibleGroups = computed(() => {
  const keyword = search.value.trim().toLowerCase().replace(/^0x/, "");
  return groups.value
    .map((group) => ({
      name: group.name,
      entries: keyword
        ? group.entries.filter((entry) =>
            entry.tgi.instance.toString(16).toLowerCase().includes(keyword),
          )
        : group.entries,
    }))
    .filter((group) => group.entries.length);
});

function toggleCollapse(name: string) {
  const next = new Set(collapsed.value);
  if (next.has(name)) next.delete(name);
  else next.add(name);
  collapsed.value = next;
}

const hex32 = (value: number) =>
  `0x${(value >>> 0).toString(16).padStart(8, "0").toUpperCase()}`;

function lotKey(entry: LotEntry) {
  return `${entry.packageId}:${entry.tgi.typeId}:${entry.tgi.group}:${entry.tgi.instance}`;
}

const appliedKey = ref("");
function apply(entry: LotEntry) {
  emit("apply", { packageId: entry.packageId, tgi: entry.tgi });
  appliedKey.value = lotKey(entry);
}
</script>

<template>
  <div class="lot-library" :aria-label="t('studio.assetWorkbench.lotLibraryTab')">
    <p v-if="!packages.opened.length" class="library-note">
      {{ t("studio.assetWorkbench.lotLibraryNeedPackages") }}
    </p>
    <template v-else>
      <div class="library-toolbar">
        <input
          v-model="search"
          type="search"
          class="library-search"
          :placeholder="t('studio.assetWorkbench.lotLibrarySearch')"
          :aria-label="t('studio.assetWorkbench.lotLibrarySearch')"
        />
        <button
          type="button"
          class="library-rescan"
          :disabled="busy"
          :title="t('studio.assetWorkbench.lotLibraryRescan')"
          :aria-label="t('studio.assetWorkbench.lotLibraryRescan')"
          @click="scan"
        >
          <FIcon :name="busy ? 'Loader2' : 'RefreshCw'" :size="13" aria-label="" />
        </button>
      </div>
      <p v-if="error" class="library-error" role="alert">{{ error }}</p>
      <p v-else-if="busy && !groups.length" class="library-note">
        <FSpinner size="sm" :label="t('common.loading')" />
      </p>
      <p v-else-if="!visibleGroups.length" class="library-note">
        {{ t("studio.assetWorkbench.lotLibraryEmpty") }}
      </p>
      <div class="library-groups">
        <div v-for="group in visibleGroups" :key="group.name" class="library-group">
          <button
            type="button"
            class="library-group-head"
            @click="toggleCollapse(group.name)"
          >
            <span>{{ group.name }}</span>
            <span class="library-count">{{ group.entries.length }}</span>
          </button>
          <div v-show="!collapsed.has(group.name)" class="library-cards">
            <div
              v-for="entry in group.entries.slice(0, PER_GROUP_CAP)"
              :key="lotKey(entry)"
              class="lot-card"
            >
              <AssetLotDiagram :package-id="entry.packageId" :tgi="entry.tgi" />
              <div class="lot-card-meta">
                <code class="lot-card-tgi" :title="hex32(entry.tgi.instance)">
                  {{ hex32(entry.tgi.instance) }}
                </code>
                <span v-if="entry.lotSize" class="lot-card-size">
                  {{ entry.lotSize[0] }} × {{ entry.lotSize[1] }} m
                </span>
                <button
                  type="button"
                  class="lot-apply"
                  :disabled="!editEnabled || props.editMode !== 'draft'"
                  :class="{ applied: appliedKey === lotKey(entry) }"
                  :title="
                    props.editMode !== 'draft'
                      ? t('studio.assetWorkbench.lotApplyNativeHint')
                      : t('studio.assetWorkbench.lotApplyDraftHint')
                  "
                  @click="apply(entry)"
                >
                  <FIcon
                    :name="appliedKey === lotKey(entry) ? 'Check' : 'Download'"
                    :size="12"
                    aria-label=""
                  />
                  {{
                    appliedKey === lotKey(entry)
                      ? t("studio.assetWorkbench.lotApplied")
                      : t("studio.assetWorkbench.lotApply")
                  }}
                </button>
              </div>
            </div>
            <p
              v-if="group.entries.length > PER_GROUP_CAP"
              class="library-note"
            >
              {{ t("studio.assetWorkbench.lotLibraryCapped", { count: PER_GROUP_CAP }) }}
            </p>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.lot-library {
  display: flex;
  flex-direction: column;
  gap: 8px;
  height: 100%;
  min-height: 0;
  padding: 8px;
}
.library-note {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
  padding: 8px;
  text-align: center;
}
.library-toolbar {
  display: flex;
  flex: none;
  gap: 5px;
}
.library-search {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  flex: 1;
  font: inherit;
  font-size: 11.5px;
  min-height: 27px;
  min-width: 0;
  padding: 0 9px;
}
.library-rescan {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 27px;
  min-width: 27px;
}
.library-rescan:hover:not(:disabled) {
  background: var(--surface-hover);
  color: var(--foreground);
}
.library-error {
  color: var(--danger);
  font-size: 11px;
  margin: 0;
  overflow-wrap: anywhere;
  padding: 0 4px;
}
.library-groups {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.library-group {
  margin-bottom: 7px;
}
.library-group-head {
  align-items: center;
  background: var(--surface-elevated);
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 11px;
  font-weight: 600;
  gap: 6px;
  justify-content: space-between;
  min-height: 27px;
  padding: 0 8px;
  width: 100%;
}
.library-count {
  background: var(--surface-hover);
  border-radius: 999px;
  color: var(--muted-foreground);
  font-size: 10px;
  padding: 0 7px;
}
.library-cards {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin-top: 6px;
}
.lot-card {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  gap: 5px;
  min-width: 0;
  padding: 5px;
}
.lot-card-meta {
  align-items: center;
  display: flex;
  gap: 5px;
}
.lot-card-tgi {
  color: var(--muted-foreground);
  flex: 1;
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.lot-card-size {
  color: var(--muted-foreground);
  flex: none;
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}
.lot-apply {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  flex: none;
  font: inherit;
  font-size: 10px;
  gap: 3px;
  min-height: 22px;
  padding: 0 6px;
  white-space: nowrap;
}
.lot-apply:hover:not(:disabled) {
  border-color: var(--primary);
  color: var(--primary);
}
.lot-apply:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.lot-apply.applied {
  border-color: var(--success);
  color: var(--success);
}
</style>
