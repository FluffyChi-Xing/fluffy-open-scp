<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

/**
 * 右栏"地块"页签：draft（外部导入模型）模式下编辑地块 footprint 尺寸
 * （视口地面矩形 + 尺度标注随之变化）；native 模式尺寸来自游戏 property，
 * 只读展示。
 */
const props = defineProps<{
  lotSize: [number, number] | null;
  editMode: "native" | "draft";
}>();
const emit = defineEmits<{ "set-size": [size: [number, number]] }>();

const { t } = useI18n();
const width = computed({
  get: () => props.lotSize?.[0] ?? 0,
  set: (value: number) => {
    if (!props.lotSize || props.editMode !== "draft") return;
    emit("set-size", [value, props.lotSize[1]]);
  },
});
const depth = computed({
  get: () => props.lotSize?.[1] ?? 0,
  set: (value: number) => {
    if (!props.lotSize || props.editMode !== "draft") return;
    emit("set-size", [props.lotSize[0], value]);
  },
});
</script>

<template>
  <div class="lot-panel" :aria-label="t('studio.assetWorkbench.lotTab')">
    <template v-if="lotSize">
      <label class="lot-field">
        <span>{{ t("studio.assetWorkbench.lotWidth") }}</span>
        <input
          v-model.number="width"
          type="number"
          min="4"
          :disabled="editMode !== 'draft'"
        />
      </label>
      <label class="lot-field">
        <span>{{ t("studio.assetWorkbench.lotDepth") }}</span>
        <input
          v-model.number="depth"
          type="number"
          min="4"
          :disabled="editMode !== 'draft'"
        />
      </label>
      <p v-if="editMode === 'draft'" class="lot-hint">
        {{ t("studio.assetWorkbench.lotDraftHint") }}
      </p>
      <p v-else class="lot-hint">
        {{ t("studio.assetWorkbench.lotNativeHint") }}
      </p>
    </template>
    <p v-else class="lot-hint">{{ t("studio.assetWorkbench.lotEmpty") }}</p>
  </div>
</template>

<style scoped>
.lot-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px;
}
.lot-field {
  color: var(--muted-foreground);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 4px;
}
.lot-field input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  min-height: 27px;
  padding: 0 8px;
}
.lot-field input:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.lot-hint {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.5;
  margin: 0;
}
</style>
