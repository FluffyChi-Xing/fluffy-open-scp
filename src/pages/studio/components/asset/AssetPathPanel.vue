<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSelect from "@/components/ui/FSelect.vue";
import type { PathPointUnit } from "@/api/tauri";
import {
  pathPointPositions,
  validateWorkbenchPath,
  type WorkbenchPath,
} from "./path-planning/pathModel";
import {
  DEFAULT_PATH_TYPE,
  PATH_TYPES,
  entryHexLabel,
  pathSemantics,
  pathTypeSpec,
  type PathTypeId,
} from "./path-planning/pathTypes";

/**
 * 路径规划面板（2026-11-07 类型化重设计）：作者只选"是什么路"
 * （人行道/车辆引道/自定义 entry），entry/scope/roadConnect 由构建器的
 * path_catalog 处理器换算写列。不再要求选包/选 property。
 *
 * 纯函数边界：类型配方在 path-planning/pathTypes.ts，校验在 pathModel.ts，
 * 本组件只做展示与事件转发。
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
  "begin-draw": [type: PathTypeId];
  "finish-draw": [];
  "undo-point": [];
  focus: [id: string | null];
  update: [id: string, patch: Partial<WorkbenchPath>];
  remove: [id: string];
}>();

const { t } = useI18n();

/** 绘制类型：开始绘制时随路径实体一起落库（引擎语义由类型决定）。 */
const selectedType = ref<PathTypeId>(DEFAULT_PATH_TYPE);
/** 只有这两类是"常规"选择；custom 在路径行内作为高级项出现。 */
const drawTypes = computed(() =>
  PATH_TYPES.filter((spec) => spec.id !== "custom"),
);
const typeOptions = computed(() =>
  PATH_TYPES.map((spec) => ({ label: t(spec.labelKey), value: spec.id })),
);

function onBeginDraw(): void {
  if (!props.editEnabled) return;
  emit("begin-draw", selectedType.value);
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
  typeLabel: string;
  entryLabel: string;
  scopeLabel: string;
  roadConnect: boolean;
}

const rows = computed<PathRow[]>(() =>
  props.paths.map((path) => {
    const positions = pathPointPositions(path, byId.value);
    const units = path.pointIds.map((id) => byId.value.get(id) ?? null);
    const issues = validateWorkbenchPath(path, units).map((code) =>
      t(`studio.assetWorkbench.pathIssue.${code}`),
    );
    const spec = pathTypeSpec(path.type);
    const semantics = pathSemantics(path.type, path.entry);
    return {
      path,
      points: positions.filter(Boolean).length,
      issues,
      typeLabel: t(spec.labelKey),
      entryLabel: entryHexLabel(semantics.entry),
      scopeLabel: t(
        semantics.scope === "global"
          ? "studio.assetWorkbench.pathScopeGlobal"
          : "studio.assetWorkbench.pathScopeLocal",
      ),
      roadConnect: semantics.roadConnect,
    };
  }),
);
</script>

<template>
  <div class="path-panel" :aria-label="t('studio.assetWorkbench.pathPanel')">
    <p v-if="!editEnabled" class="path-note">
      {{ t("studio.assetWorkbench.unlockFirst") }}
    </p>
    <template v-else>
      <!-- 类型卡：选"是什么路" → 开始绘制 -->
      <section class="type-card">
        <div class="type-head">
          <span>{{ t("studio.assetWorkbench.pathTypeTitle") }}</span>
        </div>
        <div class="type-grid" role="radiogroup">
          <button
            v-for="spec in drawTypes"
            :key="spec.id"
            type="button"
            role="radio"
            class="type-row"
            :class="{ selected: selectedType === spec.id }"
            :aria-checked="selectedType === spec.id"
            @click="selectedType = spec.id"
          >
            <FIcon :name="spec.icon" :size="14" aria-label="" />
            <span class="type-text">
              <span class="type-name">{{ t(spec.labelKey) }}</span>
              <span class="type-hint">{{ t(spec.hintKey) }}</span>
            </span>
            <span class="type-entry">{{ entryHexLabel(spec.entry) }}</span>
          </button>
        </div>
        <p class="path-hint type-note">
          {{ t("studio.assetWorkbench.pathTypeHint") }}
        </p>
        <div class="path-actions">
          <button
            type="button"
            class="path-btn primary draw"
            :class="{ drawing }"
            @click="drawing ? emit('finish-draw') : onBeginDraw()"
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

      <!-- 已有路径：类型 + 派生语义（只读展示） -->
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
        <label class="path-field">
          <span>{{ t("studio.assetWorkbench.pathTypeTitle") }}</span>
          <FSelect
            class="path-input"
            :model-value="row.path.type"
            :options="typeOptions"
            @update:model-value="
              emit('update', row.path.id, {
                type: String($event) as PathTypeId,
              })
            "
          />
        </label>
        <div v-if="row.path.type === 'custom'" class="path-field">
          <span>{{ t("studio.assetWorkbench.pathEntry") }}</span>
          <input
            class="path-input"
            :value="row.path.entry ?? ''"
            :placeholder="t('studio.assetWorkbench.pathEntryPlaceholder')"
            :aria-label="t('studio.assetWorkbench.pathEntry')"
            @change="
              emit('update', row.path.id, {
                entry: ($event.target as HTMLInputElement).value.trim() || null,
              })
            "
          />
        </div>
        <p class="path-semantics">
          <FIcon name="Cpu" :size="11" aria-label="" />
          <span>{{ t("studio.assetWorkbench.pathSemantics") }}</span>
          <code>{{ row.typeLabel }}</code>
          <code>{{ row.entryLabel }}</code>
          <span class="path-badge">{{ row.scopeLabel }}</span>
          <span v-if="row.roadConnect" class="path-badge">
            {{ t("studio.assetWorkbench.pathRoadConnect") }}
          </span>
        </p>
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
.type-card {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding: 8px;
}
.type-head {
  color: var(--muted-foreground);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.type-grid {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.type-row {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  gap: 8px;
  padding: 7px 8px;
  text-align: left;
}
.type-row:hover {
  border-color: var(--border-strong);
}
.type-row.selected {
  border-color: var(--primary);
}
.type-text {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}
.type-name {
  font-size: 11.5px;
}
.type-hint {
  color: var(--muted-foreground);
  font-size: 10px;
}
.type-entry {
  color: var(--muted-foreground);
  font-family: var(--font-mono, monospace);
  font-size: 10px;
}
.type-note {
  color: var(--muted-foreground);
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
.path-semantics {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex-wrap: wrap;
  font-size: 10px;
  gap: 5px;
  margin: 0;
}
.path-semantics code {
  color: var(--foreground);
  font-family: var(--font-mono, monospace);
}
.path-badge {
  background: var(--surface-hover);
  border-radius: 999px;
  padding: 0 6px;
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
