<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSelect from "@/components/ui/FSelect.vue";
import { Switch } from "@/components/ui/switch";
import { useToast } from "@/composables/useToast";
import { tauriApi } from "@/api";
import type { PathPointUnit, PathSourceSummary } from "@/api/tauri";
import { useGamePackagesStore } from "@/stores/gamePackages";
import {
  pathPointPositions,
  validateWorkbenchPath,
  type WorkbenchPath,
  type WorkbenchPathSource,
} from "./path-planning/pathModel";
import {
  entryDisplayLabel,
  selectPathSourcePage,
  sourceDisplayLabel,
} from "./path-planning/sources";

/**
 * 路径规划面板（2026-10-28 重设计）：目标卡（选包 → 筛选带路径的
 * property → 看参数 → 选中/开始绘制）+ 已有路径列表。
 * 用户不接触 path entry 概念：entry 由选中目标的 property 自动回填，
 * WorkbenchPath.source 记录出处（重开回显）。筛选/分页纯函数在
 * path-planning/sources.ts。
 */
defineOptions({ name: "AssetPathPanel" });
const props = defineProps<{
  paths: WorkbenchPath[];
  activePathId: string | null;
  focusedPathId: string | null;
  drawing: boolean;
  editEnabled: boolean;
  /** 视口当前展示的 pathPoint 单元（校验与点位计数用）。 */
  pathPoints: PathPointUnit[];
}>();
const emit = defineEmits<{
  "begin-draw": [target: { entry: string | null; source: WorkbenchPathSource }];
  "finish-draw": [];
  "undo-point": [];
  focus: [id: string | null];
  update: [id: string, patch: Partial<WorkbenchPath>];
  remove: [id: string];
}>();

const { t } = useI18n();
const toast = useToast();
const packages = useGamePackagesStore();

// ---- 目标卡状态 ----
const packageOptions = computed(() =>
  packages.opened.map((opened) => {
    const label =
      opened.package.path.split(/[\\/]/).pop() ?? opened.package.path;
    return { label, value: String(opened.package.packageId) };
  }),
);
const packageId = ref("");
const sources = ref<PathSourceSummary[]>([]);
const sourcesLoading = ref(false);
const sourceFilter = ref("");
const sourcePage = ref(1);
const PAGE_SIZE = 5;
const selectedSource = ref<PathSourceSummary | null>(null);

const sourcePageView = computed(() =>
  selectPathSourcePage(
    sources.value,
    sourceFilter.value,
    sourcePage.value,
    PAGE_SIZE,
  ),
);

async function onPackageChange(value: string | number): Promise<void> {
  packageId.value = String(value);
  selectedSource.value = null;
  sources.value = [];
  sourcePage.value = 1;
  const id = Number(value);
  if (!Number.isFinite(id)) return;
  sourcesLoading.value = true;
  try {
    sources.value = await tauriApi.packages.listPathSources(id);
  } catch {
    sources.value = [];
  } finally {
    sourcesLoading.value = false;
  }
}

function selectSource(row: PathSourceSummary): void {
  selectedSource.value = row;
  sourcePage.value = sourcePageView.value.page;
}

function sourceTitle(row: PathSourceSummary): string {
  return row.name || `0x${row.instance.toString(16).toUpperCase()}`;
}

function sourceHex(row: PathSourceSummary): string {
  return `G${row.group.toString(16).toUpperCase()} I${row.instance
    .toString(16)
    .toUpperCase()}`;
}

function sourceMatches(row: PathSourceSummary): boolean {
  return (
    !!selectedSource.value &&
    selectedSource.value.packageId === row.packageId &&
    selectedSource.value.instance === row.instance &&
    selectedSource.value.group === row.group
  );
}

// ---- 目标绑定与绘制 ----
const boundTarget = ref<{
  entry: string | null;
  source: WorkbenchPathSource;
} | null>(null);

function bindTarget(): void {
  const row = selectedSource.value;
  if (!row) return;
  const source: WorkbenchPathSource = {
    packageId: row.packageId,
    typeId: row.typeId,
    group: row.group,
    instance: row.instance,
    label: row.name || undefined,
  };
  boundTarget.value = { entry: row.entryIds[0]?.toString(16) ?? null, source };
  toast.success(
    t("studio.assetWorkbench.pathTargetBound", { name: sourceTitle(row) }),
  );
}

function onBeginDraw(): void {
  if (!boundTarget.value) {
    toast.warning(t("studio.assetWorkbench.pathTargetRequired"));
    return;
  }
  emit("begin-draw", boundTarget.value);
}

// ---- 已有路径列表 ----
const byId = computed(() => {
  const map = new Map<string, PathPointUnit>();
  for (const unit of props.pathPoints) map.set(`pathPoint:${unit.index}`, unit);
  return map;
});

interface PathRow {
  path: WorkbenchPath;
  points: number;
  issues: string[];
  sourceLabel: string;
  entryLabel: string;
}

const rows = computed<PathRow[]>(() =>
  props.paths.map((path) => {
    const positions = pathPointPositions(path, byId.value);
    const units = path.pointIds.map((id) => byId.value.get(id) ?? null);
    const issues = validateWorkbenchPath(path, units).map((code) =>
      t(`studio.assetWorkbench.pathIssue.${code}`),
    );
    const sourceLabel = path.source
      ? `${sourceDisplayLabel(path.source)} · G${path.source.group
          .toString(16)
          .toUpperCase()} I${path.source.instance
          .toString(16)
          .toUpperCase()}`
      : "";
    return {
      path,
      points: positions.filter(Boolean).length,
      issues,
      sourceLabel,
      entryLabel: entryDisplayLabel(path.entry),
    };
  }),
);

const SCOPES = ["local", "regional"] as const;
</script>

<template>
  <div class="path-panel" :aria-label="t('studio.assetWorkbench.pathPanel')">
    <p v-if="!editEnabled" class="path-note">
      {{ t("studio.assetWorkbench.unlockFirst") }}
    </p>
    <template v-else>
      <!-- 目标卡：选包 → 筛选带路径的 property → 参数 → 选中/开始绘制 -->
      <section class="target-card">
        <FSelect
          class="package-select"
          :model-value="packageId"
          :options="packageOptions"
          :placeholder="t('studio.assetWorkbench.pathPickPackage')"
          :aria-label="t('studio.assetWorkbench.pathPickPackage')"
          @update:model-value="onPackageChange"
        />
        <input
          v-model="sourceFilter"
          class="path-input source-filter"
          type="search"
          :placeholder="t('studio.assetWorkbench.pathSourceFilter')"
          :aria-label="t('studio.assetWorkbench.pathSourceFilter')"
        />
        <p v-if="sourcesLoading" class="path-note">
          {{ t("common.loading") }}
        </p>
        <p v-else-if="packageId && !sourcePageView.total" class="path-note">
          {{ t("studio.assetWorkbench.pathSourceEmpty") }}
        </p>
        <div v-else class="source-list">
          <button
            v-for="row in sourcePageView.items"
            :key="`${row.packageId}:${row.group}:${row.instance}`"
            type="button"
            class="source-row"
            :class="{ selected: sourceMatches(row) }"
            @click="selectSource(row)"
          >
            <span class="source-name">{{ sourceTitle(row) }}</span>
            <span class="source-meta">
              {{ row.pathCount }}
              {{ t("studio.assetWorkbench.pathCountNoun") }}
              ·
              {{ row.pointCount }}
              {{ t("studio.assetWorkbench.pathPointNoun") }}
            </span>
          </button>
        </div>
        <div
          v-if="sourcePageView.pageCount > 1"
          class="source-pager"
          role="navigation"
        >
          <button
            type="button"
            class="path-eye"
            :disabled="sourcePageView.page <= 1"
            aria-label=""
            @click="sourcePage -= 1"
          >
            <FIcon name="ChevronLeft" :size="12" aria-label="" />
          </button>
          <span class="path-note">
            {{ sourcePageView.page }} / {{ sourcePageView.pageCount }}
          </span>
          <button
            type="button"
            class="path-eye"
            :disabled="sourcePageView.page >= sourcePageView.pageCount"
            aria-label=""
            @click="sourcePage += 1"
          >
            <FIcon name="ChevronRight" :size="12" aria-label="" />
          </button>
        </div>
        <div v-if="selectedSource" class="source-params">
          <div class="param-row">
            <span>{{ t("studio.assetWorkbench.pathParamTgi") }}</span>
            <code>{{ sourceHex(selectedSource) }}</code>
          </div>
          <div class="param-row">
            <span>{{ t("studio.assetWorkbench.pathParamGeometry") }}</span>
            <span>
              {{ selectedSource.pathCount }}
              {{ t("studio.assetWorkbench.pathCountNoun") }} ·
              {{ selectedSource.pointCount }}
              {{ t("studio.assetWorkbench.pathPointNoun") }}
            </span>
          </div>
          <div class="param-row">
            <span>{{ t("studio.assetWorkbench.pathEntry") }}</span>
            <code>
              {{
                selectedSource.entryIds
                  .map((id) => id.toString(16).toUpperCase())
                  .join(" / ") || "—"
              }}
            </code>
          </div>
        </div>
        <div class="path-actions">
          <button
            type="button"
            class="path-btn"
            :disabled="!selectedSource"
            @click="bindTarget"
          >
            <FIcon name="Check" :size="12" aria-label="" />
            {{ t("studio.assetWorkbench.pathSelectTarget") }}
          </button>
          <button
            type="button"
            class="path-btn primary draw"
            :class="{ drawing }"
            @click="onBeginDraw"
          >
            <FIcon
              :name="drawing ? 'Square' : 'Spline'"
              :size="12"
              aria-label=""
            />
            {{
              drawing
                ? t("studio.assetWorkbench.pathFinish")
                : t("studio.assetWorkbench.pathDraw")
            }}
          </button>
          <button
            type="button"
            class="path-btn"
            :disabled="!drawing"
            :title="t('studio.assetWorkbench.pathUndoPoint')"
            @click="emit('undo-point')"
          >
            <FIcon name="Undo2" :size="12" aria-label="" />
          </button>
        </div>
        <p v-if="drawing" class="path-hint" role="status">
          {{ t("studio.assetWorkbench.pathDrawHint") }}
        </p>
      </section>

      <!-- 已有路径：回显来源 property 与 entry -->
      <div v-for="row in rows" :key="row.path.id" class="path-card">
        <div class="path-head">
          <button
            type="button"
            class="path-eye"
            :aria-pressed="focusedPathId === row.path.id"
            :title="t('studio.assetWorkbench.pathFocus')"
            @click="
              emit('focus', focusedPathId === row.path.id ? null : row.path.id)
            "
          >
            <FIcon
              :name="focusedPathId === row.path.id ? 'Eye' : 'EyeOff'"
              :size="13"
              aria-label=""
            />
          </button>
          <input
            class="path-label"
            :value="row.path.label"
            :aria-label="t('studio.assetWorkbench.pathLabel')"
            @change="
              emit('update', row.path.id, {
                label: ($event.target as HTMLInputElement).value,
              })
            "
          />
          <span class="path-count">{{ row.points }}</span>
          <button
            type="button"
            class="path-eye"
            :title="t('studio.assetWorkbench.pathDelete')"
            @click="emit('remove', row.path.id)"
          >
            <FIcon name="Trash2" :size="12" aria-label="" />
          </button>
        </div>
        <p v-if="row.sourceLabel" class="path-source">
          <FIcon name="Box" :size="11" aria-label="" />
          {{ row.sourceLabel }}
          <template v-if="row.entryLabel">
            · Entry {{ row.entryLabel }}
          </template>
        </p>
        <div class="path-row">
          <label class="path-field">
            <span>{{ t("studio.assetWorkbench.pathScope") }}</span>
            <FSelect
              class="path-input"
              :model-value="row.path.scope"
              :options="SCOPES.map((scope) => ({ label: scope, value: scope }))"
              @update:model-value="
                emit('update', row.path.id, {
                  scope: String($event),
                })
              "
            />
          </label>
          <div class="path-check">
            <Switch
              :model-value="row.path.roadConnect"
              :disabled="!editEnabled"
              :aria-label="t('studio.assetWorkbench.pathRoadConnect')"
              @update:model-value="
                emit('update', row.path.id, {
                  roadConnect: Boolean($event),
                })
              "
            />
            <span>{{ t("studio.assetWorkbench.pathRoadConnect") }}</span>
          </div>
        </div>
        <ul v-if="row.issues.length" class="path-issues">
          <li v-for="issue in row.issues" :key="issue" class="path-issue">
            {{ issue }}
          </li>
        </ul>
      </div>
      <p v-if="!paths.length" class="path-note">
        {{ t("studio.assetWorkbench.pathEmpty") }}
      </p>
    </template>
  </div>
</template>

<style scoped>
.path-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px;
}
.target-card {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding: 8px;
}
.package-select {
  width: 100%;
}
.package-select :deep(.f-select) {
  background: var(--surface);
  font-size: 11px;
  min-height: 26px;
  padding: 0 6px;
}
.source-filter {
  width: 100%;
}
.source-list {
  display: flex;
  flex-direction: column;
  gap: 3px;
  max-height: 168px;
  overflow-y: auto;
}
.source-row {
  align-items: flex-start;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  font: inherit;
  gap: 1px;
  padding: 5px 7px;
  text-align: left;
}
.source-row:hover {
  border-color: var(--border-strong);
}
.source-row.selected {
  border-color: var(--primary);
}
.source-name {
  font-size: 11px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.source-meta {
  color: var(--muted-foreground);
  font-size: 10px;
}
.source-pager {
  align-items: center;
  display: flex;
  gap: 8px;
  justify-content: center;
}
.source-params {
  background: var(--surface);
  border: 1px dashed var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 6px 7px;
}
.param-row {
  display: flex;
  font-size: 10.5px;
  gap: 6px;
  justify-content: space-between;
}
.param-row > span:first-child {
  color: var(--muted-foreground);
}
.param-row code {
  font-family: var(--font-mono, monospace);
}
.path-actions {
  display: flex;
  gap: 5px;
}
.path-btn {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 4px;
  justify-content: center;
  min-height: 26px;
  padding: 0 8px;
}
.path-btn.primary {
  flex: 1;
}
.path-btn.primary.draw {
  background: var(--primary);
  border-color: var(--primary);
  color: var(--primary-foreground);
}
.path-btn.primary.drawing {
  background: var(--surface);
  color: var(--primary);
}
.path-btn:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.path-hint {
  color: var(--primary);
  font-size: 10.5px;
  margin: 0;
}
.path-note {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
  padding: 4px 2px;
}
.path-card {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 7px;
}
.path-head {
  align-items: center;
  display: flex;
  gap: 5px;
}
.path-eye {
  background: none;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  padding: 2px;
}
.path-eye[aria-pressed="true"] {
  color: var(--primary);
}
.path-label {
  background: none;
  border: 0;
  color: var(--foreground);
  flex: 1;
  font: inherit;
  font-size: 11.5px;
  min-width: 0;
}
.path-count {
  background: var(--surface-hover);
  border-radius: 999px;
  color: var(--muted-foreground);
  font-size: 10px;
  padding: 0 7px;
}
.path-source {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 10px;
  gap: 4px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.path-field {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.path-field > span {
  color: var(--muted-foreground);
  font-size: 10px;
}
.path-input {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 11px;
  min-height: 24px;
  min-width: 0;
  padding: 0 6px;
}
.path-field :deep(.f-select) {
  background: var(--surface);
  font-size: 11px;
  min-height: 24px;
  padding: 0 6px;
}
.path-row {
  align-items: flex-end;
  display: flex;
  gap: 8px;
}
.path-check {
  align-items: center;
  display: flex;
  font-size: 10.5px;
  gap: 6px;
  margin-bottom: 1px;
  white-space: nowrap;
}
.path-issues {
  display: flex;
  flex-direction: column;
  gap: 2px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.path-issue {
  color: var(--danger, #e5484d);
  font-size: 10px;
}
</style>
