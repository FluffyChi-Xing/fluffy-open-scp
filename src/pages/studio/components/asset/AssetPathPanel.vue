<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSelect from "@/components/ui/FSelect.vue";
import { Switch } from "@/components/ui/switch";
import type { PathEntrySummary, PathPointUnit } from "@/api/tauri";
import {
  pathPointPositions,
  validateWorkbenchPath,
  type WorkbenchPath,
} from "./pathPlanning";

/**
 * 路径规划面板（2026-10-27）：路径列表 + entry 选择器 + 校验徽章。
 * 几何（点/切线）挂在 pathPoint 单元，这里编辑路径语义（entry/scope/
 * roadConnect）与聚焦；绘制动作由工作台壳的 Draw 模式承接。
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
  entries: PathEntrySummary[];
  entriesLoading: boolean;
}>();
const emit = defineEmits<{
  create: [];
  "begin-draw": [];
  "finish-draw": [];
  "undo-point": [];
  focus: [id: string | null];
  update: [id: string, patch: Partial<WorkbenchPath>];
  remove: [id: string];
}>();

const { t } = useI18n();

const byId = computed(() => {
  const map = new Map<string, PathPointUnit>();
  for (const unit of props.pathPoints) map.set(`pathPoint:${unit.index}`, unit);
  return map;
});

interface PathRow {
  path: WorkbenchPath;
  points: number;
  issues: string[];
  entryLabel: string;
}

const rows = computed<PathRow[]>(() =>
  props.paths.map((path) => {
    const positions = pathPointPositions(path, byId.value);
    const units = path.pointIds.map((id) => byId.value.get(id) ?? null);
    const issues = validateWorkbenchPath(path, units).map((code) =>
      t(`studio.assetWorkbench.pathIssue.${code}`),
    );
    const entry = props.entries.find(
      (candidate) =>
        candidate.instance.toString(16).toLowerCase() ===
        (path.entry ?? "").toLowerCase(),
    );
    const entryLabel = entry
      ? entry.transports.join(" / ") || `0x${entry.instance.toString(16)}`
      : "";
    return { path, points: positions.filter(Boolean).length, issues, entryLabel };
  }),
);

const entrySuggestions = computed(() => props.entries.slice(0, 32));

function commitEntry(path: WorkbenchPath, raw: string): void {
  const value = raw.trim().toLowerCase().replace(/^0x/, "");
  emit("update", path.id, { entry: value || null });
}

const SCOPES = ["local", "regional"] as const;
</script>

<template>
  <div class="path-panel" :aria-label="t('studio.assetWorkbench.pathPanel')">
    <p v-if="!editEnabled" class="path-note">
      {{ t("studio.assetWorkbench.unlockFirst") }}
    </p>
    <template v-else>
      <div class="path-actions">
        <button
          type="button"
          class="path-btn primary"
          :class="{ drawing }"
          @click="drawing ? emit('finish-draw') : emit('begin-draw')"
        >
          <FIcon :name="drawing ? 'Square' : 'Spline'" :size="12" aria-label="" />
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
          <span>{{ t("studio.assetWorkbench.pathEntry") }}</span>
          <input
            class="path-input"
            :value="row.path.entry ?? ''"
            :placeholder="t('studio.assetWorkbench.pathEntryPlaceholder')"
            list="workbench-path-entries"
            @change="
              commitEntry(
                row.path,
                ($event.target as HTMLInputElement).value,
              )
            "
          />
        </label>
        <p v-if="row.entryLabel" class="path-entry-label">{{ row.entryLabel }}</p>
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
      <datalist id="workbench-path-entries">
        <option
          v-for="entry in entrySuggestions"
          :key="entry.instance"
          :value="entry.instance.toString(16)"
        >
          {{ entry.transports.join(" / ") }}
        </option>
      </datalist>
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
.path-actions {
  display: flex;
  gap: 5px;
}
.path-btn {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 4px;
  min-height: 26px;
  padding: 0 8px;
}
.path-btn.primary {
  flex: 1;
}
.path-btn.drawing {
  border-color: var(--primary);
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
.path-row {
  align-items: flex-end;
  display: flex;

/* FSelect 默认 38px 高（表单级），路径卡内对齐 24px 输入的紧凑档 */
.path-field :deep(.f-select) {
  background: var(--surface);
  font-size: 11px;
  min-height: 24px;
  padding: 0 6px;
}
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
.path-entry-label {
  color: var(--muted-foreground);
  font-size: 10px;
  margin: -2px 0 0;
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
