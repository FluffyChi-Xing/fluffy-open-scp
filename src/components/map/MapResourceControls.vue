<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { resourceColor } from "@/lib/region-map";

defineProps<{
  options: { kind: string; label: string; available: boolean }[];
  disabled?: boolean;
}>();
const selected = defineModel<string | null>({ required: true });
const opacity = defineModel<number>("opacity", { required: true });
const { t } = useI18n();
</script>

<template>
  <div class="resource-controls">
    <div
      class="resource-switches"
      role="group"
      :aria-label="t('studio.map.resourceControls')"
    >
      <span class="resource-label">{{ t("studio.map.resourceControls") }}</span>
      <button
        type="button"
        :disabled="disabled"
        :aria-pressed="selected === null"
        @click="selected = null"
      >
        {{ t("studio.map.layerOff") }}
      </button>
      <button
        v-for="option in options"
        :key="option.kind"
        type="button"
        :disabled="disabled || !option.available"
        :aria-pressed="selected === option.kind"
        :title="
          option.available ? option.label : t('studio.map.resourceUnavailable')
        "
        @click="selected = selected === option.kind ? null : option.kind"
      >
        {{ option.label }}
      </button>
    </div>
    <div v-if="selected" class="resource-legend">
      <span>{{ t("studio.map.resourceLow") }}</span>
      <span
        class="resource-ramp"
        :style="{
          background: `linear-gradient(to right, #e5e5e5, ${resourceColor(selected)})`,
        }"
      ></span>
      <span>{{ t("studio.map.resourceHigh") }}</span>
      <label
        >{{ t("studio.map.resourceOpacity")
        }}<input
          v-model.number="opacity"
          type="range"
          min="0"
          max="1"
          step="0.05"
      /></label>
    </div>
  </div>
</template>

<style scoped>
.resource-controls {
  display: grid;
  gap: 8px;
  min-width: 0;
}
.resource-switches {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 3px;
}
.resource-label {
  font-size: 12px;
  color: var(--muted-foreground);
  margin-inline-end: 8px;
}
button {
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--muted-foreground);
  font: inherit;
  font-size: 12px;
  min-height: 28px;
  padding: 3px 9px;
  cursor: pointer;
}
button:hover:not(:disabled) {
  background: var(--surface-hover);
  color: var(--foreground);
}
button[aria-pressed="true"] {
  border-color: var(--border);
  background: var(--surface-elevated);
  color: var(--foreground);
}
button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
button:focus-visible,
input:focus-visible {
  outline: 2px solid var(--brand);
  outline-offset: 2px;
}
.resource-legend {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  flex-wrap: wrap;
  font-size: 11px;
  color: var(--muted-foreground);
}
.resource-ramp {
  width: 90px;
  height: 6px;
  border-radius: 3px;
}
label {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-inline-start: 12px;
}
input {
  width: 90px;
  accent-color: var(--brand);
}
</style>
