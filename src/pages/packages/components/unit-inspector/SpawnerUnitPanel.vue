<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { LotUnitDto } from "@/api/tauri";
import {
  ANCHOR_KIND_LABEL,
  CAMERA_ANCHOR_ID,
  UI_ANCHOR_ID,
  anchorKey,
  anchorKindOf,
  canUseAnchorName,
  describeSpawners,
  fnv1NameId,
  formatAnchorId,
  readAnchorId,
  readAnchorPosition,
  spawnerIssuesFor,
} from "./spawnerAnchor";

/**
 * 生成器（spawner）属性面板：资产编辑器「属性」页签 + Property Viewer 共用。
 *
 * 只暴露引擎真实存在的字段：名字 id（写作 FNV-1 哈希）与局部变换。
 * 人数 / 角色外观不属于锚点（见 docs/re/spawner-render-and-assembly-20261101.md §3-§4），
 * 面板里只做解释，不提供假开关。
 */
const props = withDefaults(
  defineProps<{
    unit: LotUnitDto | null;
    units: readonly LotUnitDto[];
    /** 编辑门控：未解锁（含 Property Viewer 只读态）时输入禁用。 */
    editEnabled?: boolean;
  }>(),
  { editEnabled: false },
);
const emit = defineEmits<{
  "update-fields": [id: string, patch: Record<string, unknown>];
}>();

const { t } = useI18n();

const spawner = computed(() =>
  props.unit && props.unit.kind === "spawner" ? props.unit : null,
);
const anchorId = computed(() => readAnchorId(spawner.value));
const kind = computed(() => anchorKindOf(anchorId.value));
const position = computed(() => readAnchorPosition(spawner.value));
const unitKey = computed(() =>
  spawner.value ? `spawner:${spawner.value.index}` : "",
);
const issues = computed(() =>
  spawner.value ? spawnerIssuesFor(props.units, unitKey.value) : [],
);
const duplicateCount = computed(() => {
  const id = anchorId.value;
  if (id === null) return 0;
  return describeSpawners(props.units).filter(
    (anchor) => anchor.anchorId === id,
  ).length;
});
const editable = computed(() => props.editEnabled);

const draftName = ref("");
watch(
  () => [unitKey.value, anchorId.value],
  () => {
    draftName.value = "";
  },
  { immediate: true },
);

const nameError = computed(() => {
  const name = draftName.value.trim();
  if (!name) return "";
  if (!canUseAnchorName(props.units, name, unitKey.value))
    return t("spawnerPanel.issue.anchor-duplicate");
  return "";
});
const canCommit = computed(
  () => editable.value && draftName.value.trim().length > 0 && !nameError.value,
);

function commitName() {
  if (!canCommit.value) return;
  const instance = fnv1NameId(draftName.value.trim());
  emit("update-fields", unitKey.value, { id: anchorKey(instance) });
  draftName.value = "";
}

function applyPreset(instance: number | null) {
  if (!editable.value) return;
  emit("update-fields", unitKey.value, {
    id: instance === null ? null : anchorKey(instance),
  });
  draftName.value = "";
}

const legacyFields = computed(() => {
  const unit = spawner.value;
  if (!unit) return [];
  return [
    { key: "count", value: unit.count },
    { key: "countRandom", value: unit.countRandom },
    { key: "agent", value: unit.agent ? unit.agent.instance : null },
  ].filter((row) => row.value !== null && row.value !== undefined);
});
</script>

<template>
  <section class="spawner-panel" :aria-label="t('spawnerPanel.title')">
    <template v-if="spawner">
      <header class="sp-head">
        <FIcon name="MapPin" :size="14" aria-label="" />
        <strong>{{ t("spawnerPanel.title") }}</strong>
        <span class="sp-kind" :class="`kind-${kind}`">
          {{ t(ANCHOR_KIND_LABEL[kind]) }}
        </span>
      </header>
      <p class="sp-intro">{{ t("spawnerPanel.intro") }}</p>

      <div class="sp-row">
        <span class="sp-label">{{ t("spawnerPanel.anchorId") }}</span>
        <code class="sp-id">{{
          anchorId === null ? t("spawnerPanel.anchorUnnamed") : formatAnchorId(anchorId)
        }}</code>
      </div>
      <p class="sp-kind-hint">{{ t(`spawnerPanel.anchorKindHint.${kind}`) }}</p>

      <div class="sp-field">
        <label :for="`spawner-name-${spawner.index}`">
          {{ t("spawnerPanel.anchorName") }}
        </label>
        <div class="sp-input-row">
          <input
            :id="`spawner-name-${spawner.index}`"
            v-model="draftName"
            type="text"
            spellcheck="false"
            data-testid="spawner-name-input"
            :disabled="!editable"
            :placeholder="t('spawnerPanel.anchorNamePlaceholder')"
            :aria-invalid="Boolean(nameError)"
            @keydown.enter.prevent="commitName"
          />
          <button
            type="button"
            class="sp-primary"
            data-testid="spawner-commit-name"
            :disabled="!canCommit"
            @click="commitName"
          >
            {{ t("spawnerPanel.commitName") }}
          </button>
        </div>
        <p v-if="nameError" class="sp-error" role="alert">{{ nameError }}</p>
        <p v-else class="sp-hint">{{ t("spawnerPanel.anchorNameHint") }}</p>
        <p v-if="anchorId === null" class="sp-hint">
          {{ t("spawnerPanel.anchorUnnamedHint") }}
        </p>
        <p v-else-if="duplicateCount > 1" class="sp-error">
          {{ t("spawnerPanel.duplicateOf", { count: duplicateCount }) }}
        </p>
      </div>

      <div class="sp-presets">
        <button
          type="button"
          :disabled="!editable"
          @click="applyPreset(UI_ANCHOR_ID)"
        >
          {{ t("spawnerPanel.useUiAnchor") }}
        </button>
        <button
          type="button"
          :disabled="!editable"
          @click="applyPreset(CAMERA_ANCHOR_ID)"
        >
          {{ t("spawnerPanel.useCameraAnchor") }}
        </button>
        <button
          type="button"
          :disabled="!editable || anchorId === null"
          @click="applyPreset(null)"
        >
          {{ t("spawnerPanel.clearName") }}
        </button>
      </div>

      <ul v-if="issues.length" class="sp-issues" role="status">
        <li v-for="issue in issues" :key="issue.code">
          <FIcon name="CircleAlert" :size="12" aria-label="" />
          {{ t(`spawnerPanel.issue.${issue.code}`) }}
        </li>
      </ul>

      <div class="sp-field">
        <span class="sp-label">{{ t("spawnerPanel.position") }}</span>
        <code v-if="position" class="sp-id">
          {{ position.map((value) => value.toFixed(2)).join(" , ") }}
        </code>
        <span v-else class="sp-hint">{{ t("spawnerPanel.issue.transform-missing") }}</span>
        <p class="sp-hint">{{ t("spawnerPanel.positionHint") }}</p>
      </div>

      <details v-if="legacyFields.length" class="sp-legacy">
        <summary>{{ t("spawnerPanel.legacyTitle") }}</summary>
        <p class="sp-hint">{{ t("spawnerPanel.legacyHint") }}</p>
        <ul>
          <li v-for="row in legacyFields" :key="row.key">
            <code>{{ row.key }}</code><span>{{ row.value }}</span>
          </li>
        </ul>
      </details>

      <div class="sp-cast">
        <strong>{{ t("spawnerPanel.castTitle") }}</strong>
        <p class="sp-hint">{{ t("spawnerPanel.castHint") }}</p>
      </div>

      <p v-if="!editable" class="sp-hint">{{ t("spawnerPanel.readonlyHint") }}</p>
    </template>
    <p v-else class="sp-hint">{{ t("spawnerPanel.listEmpty") }}</p>
  </section>
</template>

<style scoped>
.spawner-panel {
  border-bottom: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
}
.sp-head {
  align-items: center;
  color: var(--foreground);
  display: flex;
  font-size: 12px;
  gap: 6px;
}
.sp-kind {
  border: 1px solid var(--border);
  border-radius: 999px;
  font-size: 10px;
  font-weight: 500;
  margin-inline-start: auto;
  padding: 1px 8px;
}
.sp-kind.kind-ui {
  border-color: var(--brand);
  color: var(--brand);
}
.sp-kind.kind-camera {
  border-color: #f59e0b;
  color: #f59e0b;
}
.sp-kind.kind-unnamed {
  color: var(--muted-foreground);
}
.sp-intro,
.sp-hint,
.sp-kind-hint {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.5;
  margin: 0;
}
.sp-error {
  color: var(--destructive, #ef4444);
  font-size: 11px;
  line-height: 1.5;
  margin: 0;
}
.sp-row {
  align-items: center;
  display: flex;
  gap: 8px;
  justify-content: space-between;
}
.sp-label,
.sp-field label {
  color: var(--muted-foreground);
  font-size: 11px;
}
.sp-id {
  background: var(--surface-elevated);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  padding: 2px 6px;
}
.sp-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.sp-input-row {
  display: flex;
  gap: 6px;
}
.sp-input-row input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  flex: 1;
  font: inherit;
  font-size: 11.5px;
  min-height: 27px;
  min-width: 0;
  padding: 0 8px;
}
.sp-input-row input[aria-invalid="true"] {
  border-color: var(--destructive, #ef4444);
}
.sp-primary {
  background: var(--brand);
  border: 1px solid var(--brand);
  border-radius: var(--radius-sm);
  color: #fff;
  cursor: pointer;
  font: inherit;
  font-size: 11.5px;
  padding: 0 10px;
}
.sp-presets {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.sp-presets button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
  padding: 4px 8px;
}
.sp-presets button:disabled,
.sp-primary:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.sp-issues {
  background: color-mix(in srgb, #ef4444 10%, transparent);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 3px;
  list-style: none;
  margin: 0;
  padding: 6px 8px;
}
.sp-issues li {
  align-items: flex-start;
  color: var(--destructive, #ef4444);
  display: flex;
  gap: 5px;
  line-height: 1.45;
}
.sp-legacy {
  border-top: 1px dashed var(--border);
  font-size: 11px;
  padding-top: 6px;
}
.sp-legacy summary {
  color: var(--muted-foreground);
  cursor: pointer;
}
.sp-legacy ul {
  display: flex;
  flex-direction: column;
  gap: 2px;
  list-style: none;
  margin: 4px 0 0;
  padding: 0;
}
.sp-legacy li {
  display: flex;
  gap: 8px;
  justify-content: space-between;
}
.sp-cast {
  border-top: 1px dashed var(--border);
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding-top: 8px;
}
.sp-cast strong {
  font-size: 11.5px;
}
</style>
