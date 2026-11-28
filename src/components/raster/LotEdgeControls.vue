<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { RasterSchema } from "@/lib/raster-editor/schema";

const props = defineProps<{ slot: number; disabled?: boolean }>();
const lot = defineModel<RasterSchema["lot"]>({ required: true });
const emit = defineEmits<{ change: [] }>();
const { t, n } = useI18n();
const fields = [
  { key: "colorHeights", label: "surface" },
  { key: "borderHeights", label: "border" },
] as const;
const width = computed(() => lot.value.borderWidths?.[props.slot] ?? 0);
const displayedWidth = shallowRef(width.value);
watch([() => props.slot, width], () => {
  displayedWidth.value = width.value;
});

function showWidth(event: Event) {
  displayedWidth.value = (event.target as HTMLInputElement).valueAsNumber;
}

function update(key: "borderWidths" | (typeof fields)[number]["key"], event: Event) {
  const input = event.target as HTMLInputElement;
  const value = input.valueAsNumber;
  const old = lot.value[key]?.[props.slot] ?? 0;
  // The native slider snaps to steps. Avoid floating-point step validation
  // rejecting values such as 0.353 when committing the drag.
  if (!Number.isFinite(Math.fround(value)) ||
      (key === "borderWidths" && (value < 0 || value > 0.5))) {
    input.value = String(old);
    return;
  }
  if (value === old) return;
  const values = lot.value[key]?.slice() ?? [0, 0, 0, 0];
  values[props.slot] = value;
  lot.value = { ...lot.value, [key]: values };
  emit("change");
}
</script>

<template>
  <fieldset class="lot-edge" :disabled="disabled">
    <legend class="lot-edge-title">{{ t("rasterEditor.lotEdge.title") }}</legend>
    <div class="lot-edge-width">
      <label class="lot-edge-row">
        {{ t("rasterEditor.lotEdge.width") }}
        <input
          class="lot-edge-slider"
          type="range"
          min="0"
          max="0.5"
          step="0.001"
          :value="displayedWidth"
          @input="showWidth"
          @change="update('borderWidths', $event)"
        />
      </label>
      <span class="lot-edge-width-value" aria-hidden="true">
        {{ n(displayedWidth, { maximumFractionDigits: 3 }) }}
      </span>
    </div>
    <label v-for="field in fields" :key="field.key" class="lot-edge-row">
      {{ t(`rasterEditor.lotEdge.${field.label}`) }}
      <input
        class="lot-edge-input"
        type="number"
        :value="lot[field.key]?.[slot] ?? 0"
        step="any"
        @change="update(field.key, $event)"
      />
    </label>
  </fieldset>
</template>

<style scoped>
.lot-edge {
  display: grid;
  gap: 6px;
  min-inline-size: 0;
  margin: 0;
  padding: 0;
  border: 0;
}
.lot-edge-title {
  margin-block-end: 6px;
  padding: 0;
  font-size: 12px;
  font-weight: 600;
}
.lot-edge-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 5.75rem);
  align-items: center;
  gap: 10px;
  color: var(--muted-foreground);
  font-size: 11px;
}
.lot-edge-input {
  inline-size: 100%;
  min-inline-size: 0;
  min-block-size: 30px;
  padding: 4px 8px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--background);
  color: var(--foreground);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}
.lot-edge-input:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
.lot-edge-input:disabled {
  opacity: 0.55;
}
.lot-edge-width {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 3.5ch;
  align-items: center;
  gap: 8px;
}
.lot-edge-width .lot-edge-row {
  grid-template-columns: auto minmax(0, 1fr);
}
.lot-edge-width-value {
  color: var(--foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  text-align: end;
}
.lot-edge-slider {
  inline-size: 100%;
  min-inline-size: 0;
  min-block-size: 30px;
  accent-color: var(--primary);
}
.lot-edge-slider:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
</style>
