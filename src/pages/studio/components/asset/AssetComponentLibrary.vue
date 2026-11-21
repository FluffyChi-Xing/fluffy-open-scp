<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useEditorPackageContext } from "@/composables/useEditorPackages";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { tauriApi } from "@/api";
import type {
  EffectAnnotation,
  PropBinCatalogEntry,
  PropUnit,
  Tgi,
} from "@/api/tauri";
import SpawnerLibraryPanel from "./PerformanceLibraryPanel.vue";
import AssetDecalLibrary from "./AssetDecalLibrary.vue";
import type { PlacePayload } from "./useWorkbenchAsset";

/**
 * 右栏"组件库"：prop bin 语义目录（后端 list_prop_bin_catalog：树/车辆/
 * 垃圾桶/代售牌子四类，固定 resourceID→模型解析）分组。无需命名模型目录
 * 或未分类条目全量扫描。lights/spawners/effects/
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
  /** 当前 lot 全部单元（生成器页签列出锚点并做重名统计）。 */
  units?: readonly import("@/api/tauri").LotUnitDto[];
  /** 当前选中单元 id（生成器页签高亮）。 */
  selectedUnitId?: string | null;
}>();
const emit = defineEmits<{
  place: [payload: PlacePayload];
  select: [unitId: string];
}>();

const { t } = useI18n();
const dependencies = useEditorPackageContext();
const loading = ref(false);
const search = ref("");
const collapsed = ref(new Set<string>());

const TABS = [
  { id: "props", icon: "Box", label: "package.groupProps" },
  { id: "decals", icon: "Image", label: "package.groupDecals" },
  { id: "spawners", icon: "MapPin", label: "package.groupSpawners" },
  { id: "effects", icon: "Zap", label: "package.groupEffects" },
  { id: "paths", icon: "Spline", label: "package.groupPaths" },
  { id: "lights", icon: "Lightbulb", label: "package.groupLights" },
] as const;
type TabId = (typeof TABS)[number]["id"];
const activeTab = ref<TabId>("props");

let catalogEpoch = 0;
async function loadCatalog() {
  if (dependencies.loading || binCatalog.value.length || loading.value) return;
  const epoch = ++catalogEpoch;
  loading.value = true;
  try {
    const result = await tauriApi.packages.listPropBinCatalog();
    if (epoch === catalogEpoch) binCatalog.value = result;
  } catch {
    if (epoch === catalogEpoch) binCatalog.value = [];
  } finally {
    if (epoch === catalogEpoch) loading.value = false;
  }
}
onMounted(loadCatalog);
watch(() => dependencies.opened.map(p => p.package.packageId).join(","), () => {
  catalogEpoch++;
  loading.value = false;
  binCatalog.value = [];
  void loadCatalog();
});
watch(
  () => props.editEnabled,
  (enabled) => {
    if (enabled) void loadCatalog();
  },
);

// ---- 效果页签：按用户创建的 effect 语义 tag 分类的目录 ----

interface EffectCatalogEntry {
  typeId: number;
  group: number;
  instance: number;
  tag: string;
  note: string;
}

const effectCatalog = ref<EffectAnnotation[]>([]);
const effectLoading = ref(false);

async function loadEffectCatalog() {
  if (effectLoading.value) return;
  effectLoading.value = true;
  try {
    effectCatalog.value = await tauriApi.effectAnnotations.list();
  } catch {
    effectCatalog.value = [];
  } finally {
    effectLoading.value = false;
  }
}

function effectHex(value: number) {
  return value.toString(16).padStart(8, "0").toUpperCase();
}

/** tag 分组目录：同 tag 下按 effect TGI 去重（多条批注保留最新一条的 note）。 */
const effectTagGroups = computed<{ name: string; entries: EffectCatalogEntry[] }[]>(
  () => {
    const keyword = search.value.trim().toLowerCase();
    const groups = new Map<string, EffectCatalogEntry[]>();
    for (const item of effectCatalog.value) {
      if (keyword) {
        const haystack = [item.tag, item.note, effectHex(item.instance)];
        if (!haystack.some((field) => field.toLowerCase().includes(keyword)))
          continue;
      }
      const key = `${item.typeId}:${item.groupId}:${item.instance}`;
      const entries = groups.get(item.tag) ?? [];
      const existing = entries.find(
        (entry) =>
          `${entry.typeId}:${entry.group}:${entry.instance}` === key,
      );
      if (existing) {
        if (item.note) existing.note = item.note;
        continue;
      }
      entries.push({
        typeId: item.typeId,
        group: item.groupId,
        instance: item.instance,
        tag: item.tag,
        note: item.note,
      });
      groups.set(item.tag, entries);
    }
    return [...groups.entries()]
      .map(([name, entries]) => ({ name, entries }))
      .sort(
        (a, b) =>
          b.entries.length - a.entries.length || a.name.localeCompare(b.name),
      );
  },
);

/** 效果页签每次激活都刷新（属性查看器里新建的 tag 即时出现）。 */
watch(activeTab, (tab) => {
  if (tab === "effects") void loadEffectCatalog();
});

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

/** 仅支持四种明确的 prop 类别。 */
const BIN_CATEGORY_ORDER: PropBinCatalogEntry["category"][] = [
  "trees",
  "vehicles",
  "waste",
  "signs",

];
const binCatalog = ref<PropBinCatalogEntry[]>([]);

/** 普查权威清单的 vanilla bin（2026-11-05 tree_hunt/car_census 锚点）：
 * 树按树种 lod 9..12（双路同 bin）、车辆全 0。缺省条目回落放置默认 bin。 */
const CENSUS_PROP_BINS: Record<number, number> = {
  0x14984c6b: 9,
  0x41c2d443: 9,
  0x14984c6a: 10,
  0xc8b42548: 10,
  0x14984c69: 11,
  0xc8b4254b: 11,
  0x14984c68: 12,
  0xc8b4254a: 12,
  0x0ab4dfe1: 0,
  0x0ab4dfe2: 0,
  0x0ab4dfe3: 0,
};

const propGroups = computed<{ name: string; entries: CatalogEntry[] }[]>(() => {
  const keyword = search.value.trim().toLowerCase();

  // 1) prop bin 语义目录：类别优先、跨包聚合（树/车辆/垃圾桶/代售牌子/杂件）
  const binEntries: CatalogEntry[] = binCatalog.value
    .map((bin): CatalogEntry => ({
      packageId: bin.modelPackageId,
      packagePath: bin.modelPackagePath,
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
  // 固定语义分类，不补入未分类模型。
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
    // 语义目录条目直写脚本 resourceID（游戏可解析）；旧命名目录条目无此
    // 字段，走直挂模型兜底。bin 取普查清单的 vanilla bin（树按树种 9..12、
    // 车辆 0——2026-11-05 car_census 锚点口径），其余类别沿用默认。
    resourceId: entry.binResourceId,
    bin:
      entry.binResourceId != null
        ? CENSUS_PROP_BINS[entry.binResourceId]
        : undefined,
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
  // 路径点必须通过 Path 面板的 Draw 模式追加到 activePath；
  // 不再把路径页签的兜底卡片接入通用组件放置管线。
  if (tab === "effects") emit("place", { kind: "effect" });
}

/** 效果目录卡：拖出即放置带 TGI 的 effect 单元（语义 tag 目录）。 */
function beginEffectDrag(event: PointerEvent, entry: EffectCatalogEntry) {
  if (!props.editEnabled) return;
  event.preventDefault();
  emit("place", {
    kind: "effect",
    effectId: {
      typeId: entry.typeId,
      group: entry.group,
      instance: entry.instance,
    } satisfies Tgi,
  });
}

function onSpawnerPlace(selection: { performancePreset?: string }) {
  emit("place", { kind: "spawner", ...selection });
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

    <AssetDecalLibrary
      v-if="activeTab === 'decals'"
      :edit-enabled="editEnabled"
      @place="decal => emit('place', { kind: 'decal', decal })"
    />
    <p v-else-if="!editEnabled" class="library-locked">
      {{ t("studio.assetWorkbench.unlockFirst") }}
    </p>
    <template v-else>
      <input
        v-if="activeTab === 'props' || activeTab === 'effects'"
        v-model="search"
        type="search"
        class="library-search"
        :placeholder="
          activeTab === 'effects'
            ? t('effectTags.librarySearch')
            : t('package.materialsSearch')
        "
        :aria-label="
          activeTab === 'effects'
            ? t('effectTags.librarySearch')
            : t('package.materialsSearch')
        "
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
              :key="`${entry.binResourceId}:${entry.packageId}:${entry.group}:${entry.instance}`"
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

      <div v-else-if="activeTab === 'spawners'" class="library-groups">
        <SpawnerLibraryPanel
          :units="units ?? []"
          :edit-enabled="editEnabled"
          :selected-unit-id="selectedUnitId ?? null"
          @place="onSpawnerPlace"
          @select="(id) => emit('select', id)"
        />
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
        <!-- 效果页签：按用户创建的语义 tag 分类；没有 tag 时必须先去属性查看器创建 -->
        <p v-if="effectLoading" class="library-note">
          <FSpinner size="sm" :label="t('common.loading')" />
        </p>
        <FEmpty
          v-else-if="!effectCatalog.length"
          icon-name="Zap"
          variant="compact"
          :title="t('effectTags.libraryEmptyTitle')"
          :desc="t('effectTags.libraryEmptyDesc')"
        />
        <template v-else>
          <div v-for="group in effectTagGroups" :key="group.name" class="library-group">
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
                :key="`${entry.typeId}:${entry.group}:${entry.instance}`"
                class="library-entry"
                :title="entry.note || `T ${effectHex(entry.typeId)} · G ${effectHex(entry.group)} · I ${effectHex(entry.instance)}`"
                @pointerdown.stop.prevent="beginEffectDrag($event, entry)"
              >
                <FIcon name="Zap" :size="18" aria-label="" />
                <span>0x{{ effectHex(entry.instance) }}</span>
              </div>
            </div>
          </div>
          <p v-if="!effectTagGroups.length" class="library-note">
            {{ t("effectTags.libraryNoMatches") }}
          </p>
        </template>
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
