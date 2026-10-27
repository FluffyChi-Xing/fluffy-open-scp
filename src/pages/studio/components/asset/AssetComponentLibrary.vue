<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { tauriApi } from "@/api";
import type { PropBinCatalogEntry, PropUnit, Tgi } from "@/api/tauri";
import {
  CATEGORY_LABEL_KEYS,
  propSemanticCategory,
  type AssetCategory,
} from "@/pages/packages/components/property-editor/assetCategories";
import type { PlacePayload } from "./useWorkbenchAsset";

/**
 * 右栏"组件库"：prop bin 语义目录（后端 list_prop_bin_catalog：树/车辆/
 * 垃圾桶/代售牌子/杂件，resourceID→模型全链解析）优先分组，命名模型目录
 * （list_model_catalog）排除已覆盖模型后按包兜底。lights/spawners/effects/
 * paths 为参数化预设。条目 pointerdown 拾起 → 视口地面落点放置。
 */
const props = defineProps<{
  editEnabled: boolean;
  /** 当前资产引用的模型条目（native 会话解析；draft 为空）。 */
  referencedModels: {
    packageId: number;
    instance: number;
    group: number;
    resourceId: number;
  }[];
  /** 树 prop 资源 id 集合（语义分类用）。 */
  propTreeIds: Set<number>;
  /** 按 resourceId 查引用 prop 模板（放置继承引擎参数）。 */
  templateOf: (resourceId: number) => PropUnit | undefined;
}>();
const emit = defineEmits<{ place: [payload: PlacePayload] }>();

const { t } = useI18n();
const loading = ref(false);
const catalog = ref<
  {
    packageId: number;
    packagePath?: string;
    instance: number;
    group: number;
    name: string;
    size: number;
  }[]
>([]);
const search = ref("");
const collapsed = ref(new Set<string>());

const TABS = [
  { id: "props", icon: "Box", label: "package.groupProps" },
  { id: "spawners", icon: "MapPin", label: "package.groupSpawners" },
  { id: "effects", icon: "Zap", label: "package.groupEffects" },
  { id: "paths", icon: "Spline", label: "package.groupPaths" },
  { id: "lights", icon: "Lightbulb", label: "package.groupLights" },
] as const;
type TabId = (typeof TABS)[number]["id"];
const activeTab = ref<TabId>("props");

async function loadCatalog() {
  if (catalog.value.length || loading.value) return;
  loading.value = true;
  try {
    const [models, bins] = await Promise.all([
      tauriApi.packages.listModelCatalog(),
      tauriApi.packages
        .listPropBinCatalog()
        .catch(() => [] as PropBinCatalogEntry[]),
    ]);
    catalog.value = models;
    binCatalog.value = bins;
  } catch {
    catalog.value = [];
  } finally {
    loading.value = false;
  }
}
onMounted(loadCatalog);
watch(
  () => props.editEnabled,
  (enabled) => {
    if (enabled) void loadCatalog();
  },
);

interface CatalogEntry {
  packageId: number;
  packagePath?: string;
  instance: number;
  group: number;
  name: string;
  size: number;
  template?: PropUnit;
  /** prop bin 语义类别（语义目录条目携带；旧命名目录条目为空）。 */
  binCategory?: PropBinCatalogEntry["category"];
  /** 语义目录 resourceId（放置继承引擎参数）。 */
  binResourceId?: number;
}

/** 语义目录固定组序：四个明确目标在前，杂件桶次之。 */
const BIN_CATEGORY_ORDER: PropBinCatalogEntry["category"][] = [
  "trees",
  "vehicles",
  "waste",
  "signs",
  "misc",
];
const binCatalog = ref<PropBinCatalogEntry[]>([]);

const propGroups = computed<{ name: string; entries: CatalogEntry[] }[]>(() => {
  const keyword = search.value.trim().toLowerCase();

  // 1) prop bin 语义目录：类别优先、跨包聚合（树/车辆/垃圾桶/代售牌子/杂件）
  const binEntries: CatalogEntry[] = binCatalog.value
    .map((bin): CatalogEntry => ({
      packageId: bin.modelPackageId,
      instance: bin.model.instance,
      group: bin.model.group,
      name: bin.name,
      size: 0,
      template: props.templateOf(bin.resourceId),
      binCategory: bin.category,
      binResourceId: bin.resourceId,
    }))
    .filter((entry) =>
      keyword ? entry.name.toLowerCase().includes(keyword) : true,
    );
  const binModelKeys = new Set(
    binEntries.map((entry) => `${entry.packageId}:${entry.instance}`),
  );

  // 2) 旧命名模型目录：排除已被语义目录覆盖的模型（按 包:实例 去重）
  const matched: CatalogEntry[] = catalog.value
    .map((entry) => ({ ...entry }))
    .filter(
      (entry) =>
        !binModelKeys.has(`${entry.packageId}:${entry.instance}`) &&
        (keyword ? entry.name.toLowerCase().includes(keyword) : true),
    );
  // 引用了但目录缺席的模型（纯 hex 名被目录噪声过滤排除）→ 兜底补入
  for (const reference of props.referencedModels) {
    if (
      matched.some(
        (entry) =>
          entry.instance === reference.instance &&
          entry.group === reference.group &&
          entry.packageId === reference.packageId,
      )
    )
      continue;
    const hexName = t("package.modelFallbackName", {
      hex: `0x${reference.instance.toString(16).toUpperCase()}`,
    });
    if (keyword && !hexName.toLowerCase().includes(keyword)) continue;
    matched.push({
      packageId: reference.packageId,
      instance: reference.instance,
      group: reference.group,
      name: hexName,
      size: 0,
    });
  }
  const legacyGroups = new Map<string, CatalogEntry[]>();
  for (const entry of matched) {
    const reference = props.referencedModels.find(
      (item) =>
        item.instance === entry.instance &&
        item.group === entry.group &&
        item.packageId === entry.packageId,
    );
    entry.template = props.templateOf(reference?.resourceId ?? -1);
    const category: AssetCategory = propSemanticCategory(
      reference?.resourceId ?? null,
      entry.name,
      props.propTreeIds,
    );
    const packageLabel =
      entry.packagePath?.split(/[\\/]/).pop() || `Package ${entry.packageId}`;
    const group = `${packageLabel} · ${t(CATEGORY_LABEL_KEYS[category])}`;
    if (!legacyGroups.has(group)) legacyGroups.set(group, []);
    legacyGroups.get(group)!.push(entry);
  }

  // 3) 语义目录固定组在前，旧命名目录按包·类别在后
  const groups: { name: string; entries: CatalogEntry[] }[] = [];
  for (const category of BIN_CATEGORY_ORDER) {
    const entries = binEntries.filter((entry) => entry.binCategory === category);
    if (!entries.length) continue;
    groups.push({
      name: t(`studio.assetWorkbench.binCategory.${category}`, {
        count: entries.length,
      }),
      entries,
    });
  }
  for (const [name, entries] of [...legacyGroups.entries()].sort((a, b) =>
    a[0].localeCompare(b[0]),
  )) {
    groups.push({ name, entries });
  }
  return groups;
});

const LIGHT_PRESETS: {
  lightType: "Point" | "Spot" | "Line";
  icon: string;
  label: string;
}[] = [
  { lightType: "Point", icon: "Lightbulb", label: "package.lightPoint" },
  { lightType: "Spot", icon: "Flashlight", label: "package.lightSpot" },
  { lightType: "Line", icon: "Minus", label: "package.lightLine" },
];

/** 非 prop 类预设（页面/载荷 kind 分离）。 */
const PLACE_PRESETS: { tab: TabId; icon: string; label: string }[] = [
  { tab: "spawners", icon: "MapPin", label: "package.groupSpawners" },
  { tab: "effects", icon: "Zap", label: "package.groupEffects" },
  { tab: "paths", icon: "Spline", label: "package.groupPaths" },
];

function toggleCollapse(name: string) {
  const next = new Set(collapsed.value);
  if (next.has(name)) next.delete(name);
  else next.add(name);
  collapsed.value = next;
}

function beginPropDrag(event: PointerEvent, entry: CatalogEntry) {
  if (!props.editEnabled) return;
  event.preventDefault();
  emit("place", {
    kind: "prop",
    packageId: entry.packageId,
    packagePath: entry.packagePath,
    tgi: {
      typeId: 0x2f4e681b,
      group: entry.group,
      instance: entry.instance,
    } satisfies Tgi,
    name: entry.name,
    template: entry.template,
  });
}

function beginLightDrag(event: PointerEvent, lightType: "Point" | "Spot" | "Line") {
  if (!props.editEnabled) return;
  event.preventDefault();
  emit("place", { kind: "light", lightType });
}

function beginPresetDrag(event: PointerEvent, tab: TabId) {
  if (!props.editEnabled) return;
  event.preventDefault();
  const kind =
    tab === "spawners" ? "spawner" : tab === "effects" ? "effect" : "pathPoint";
  emit("place", { kind });
}
</script>

<template>
  <div class="library" :aria-label="t('studio.assetWorkbench.library')">
    <div class="library-tabs" role="tablist">
      <button
        v-for="tab in TABS"
        :key="tab.id"
        type="button"
        role="tab"
        class="library-tab"
        :class="{ active: activeTab === tab.id }"
        :aria-selected="activeTab === tab.id"
        @click="activeTab = tab.id"
      >
        <FIcon :name="tab.icon" :size="12" aria-label="" />
        {{ t(tab.label) }}
      </button>
    </div>

    <p v-if="!editEnabled" class="library-locked">
      {{ t("studio.assetWorkbench.unlockFirst") }}
    </p>
    <template v-else>
      <input
        v-if="activeTab === 'props'"
        v-model="search"
        type="search"
        class="library-search"
        :placeholder="t('package.materialsSearch')"
        :aria-label="t('package.materialsSearch')"
      />
      <p v-if="loading" class="library-note">
        <FSpinner size="sm" :label="t('common.loading')" />
      </p>

      <div v-if="activeTab === 'props'" class="library-groups">
        <div v-for="group in propGroups" :key="group.name" class="library-group">
          <button
            type="button"
            class="library-group-head"
            @click="toggleCollapse(group.name)"
          >
            <span>{{ group.name }}</span>
            <span class="library-count">{{ group.entries.length }}</span>
          </button>
          <div v-show="!collapsed.has(group.name)" class="library-grid">
            <div
              v-for="entry in group.entries"
              :key="`${entry.packageId}:${entry.group}:${entry.instance}`"
              class="library-entry"
              :title="entry.name"
              @pointerdown.stop.prevent="beginPropDrag($event, entry)"
            >
              <FIcon name="Box" :size="18" aria-label="" />
              <span>{{ entry.name }}</span>
            </div>
          </div>
        </div>
        <p v-if="!propGroups.length && !loading" class="library-note">
          {{ t("package.materialsEmpty") }}
        </p>
      </div>

      <div v-else-if="activeTab === 'lights'" class="library-groups">
        <div class="library-grid">
          <div
            v-for="preset in LIGHT_PRESETS"
            :key="preset.lightType"
            class="library-entry"
            @pointerdown.stop.prevent="beginLightDrag($event, preset.lightType)"
          >
            <FIcon :name="preset.icon" :size="18" aria-label="" />
            <span>{{ t(preset.label) }}</span>
          </div>
        </div>
      </div>

      <div v-else-if="activeTab === 'paths'" class="library-groups">
        <!-- 路径页签：规划面板由工作台壳注入（路径实体状态在 useWorkbenchAsset） -->
        <slot name="paths">
          <div class="library-grid">
            <div
              class="library-entry"
              @pointerdown.stop.prevent="beginPresetDrag($event, 'paths')"
            >
              <FIcon name="Spline" :size="18" aria-label="" />
              <span>{{ t("package.groupPaths") }}</span>
            </div>
          </div>
        </slot>
      </div>

      <div v-else class="library-groups">
        <div class="library-grid">
          <div
            v-for="preset in PLACE_PRESETS"
            v-show="activeTab === preset.tab"
            :key="preset.tab"
            class="library-entry"
            @pointerdown.stop.prevent="beginPresetDrag($event, preset.tab)"
          >
            <FIcon :name="preset.icon" :size="18" aria-label="" />
            <span>{{ t(preset.label) }}</span>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.library {
  display: flex;
  flex-direction: column;
  gap: 8px;
  height: 100%;
  min-height: 0;
  padding: 8px;
}
.library-tabs {
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  gap: 2px;
  overflow-x: auto;
}
.library-tab {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 4px;
  padding: 6px 7px;
  white-space: nowrap;
}
.library-tab:hover {
  color: var(--foreground);
}
.library-tab.active {
  border-bottom-color: var(--primary);
  color: var(--foreground);
  font-weight: 600;
}
.library-locked {
  color: var(--muted-foreground);
  font-size: 11.5px;
  margin: 0;
  padding: 18px 8px;
  text-align: center;
}
.library-note {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
  padding: 8px;
  text-align: center;
}
.library-search {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  flex: none;
  font: inherit;
  font-size: 11.5px;
  min-height: 27px;
  padding: 0 9px;
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
.library-grid {
  display: grid;
  gap: 5px;
  grid-template-columns: repeat(3, 1fr);
  margin-top: 5px;
}
.library-entry {
  align-items: center;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: grab;
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 10px;
  justify-content: center;
  min-height: 58px;
  overflow: hidden;
  padding: 7px 3px;
  text-align: center;
}
.library-entry:hover {
  border-color: var(--primary);
  color: var(--primary);
}
.library-entry:active {
  cursor: grabbing;
}
.library-entry span {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
