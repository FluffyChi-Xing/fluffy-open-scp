<script setup lang="ts">
import { SliderRange, SliderRoot, SliderThumb, SliderTrack } from "reka-ui";

defineProps<{ label: string; min: number; max: number; step?: number; disabled?: boolean }>();
const value = defineModel<number>({ required: true });

function update(values: number[] | undefined) {
  if (values?.[0] !== undefined) value.value = values[0];
}
</script>

<template>
  <SliderRoot
    class="f-slider"
    :model-value="[value]"
    :min="min"
    :max="max"
    :step="step"
    :disabled="disabled"
    @update:model-value="update"
  >
    <SliderTrack class="f-slider-track"><SliderRange class="f-slider-range" /></SliderTrack>
    <SliderThumb class="f-slider-thumb" :aria-label="label" />
  </SliderRoot>
</template>

<style scoped>
.f-slider {
  position: relative;
  display: flex;
  align-items: center;
  min-height: 24px;
  touch-action: none;
}
.f-slider-track {
  position: relative;
  flex: 1;
  height: 4px;
  border-radius: var(--radius-sm);
  background: var(--border-strong);
}
.f-slider-range {
  position: absolute;
  height: 100%;
  border-radius: inherit;
  background: var(--primary);
}
.f-slider-thumb {
  display: block;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--primary);
  cursor: grab;
}
.f-slider-thumb:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 3px;
}
.f-slider[data-disabled] { opacity: 0.55; }
</style>
