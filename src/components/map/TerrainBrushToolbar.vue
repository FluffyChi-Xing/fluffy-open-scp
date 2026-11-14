<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { TerrainBrushMode } from "./terrain-brush";

defineProps<{ busy: boolean; canUndo: boolean; status: string }>();
const emit = defineEmits<{ undo: []; export: [] }>();
const mode = defineModel<TerrainBrushMode | null>("mode", { required: true });
const radius = defineModel<number>("radius", { required: true });
const strength = defineModel<number>("strength", { required: true });
const { t } = useI18n();
const modes = ["raise", "level", "lower"] as const;
</script>

<template>
  <div class="terrain-tools" role="group" :aria-label="t('studio.map.terrainBrushes')">
    <div class="brush-row">
      <button
        v-for="brush in modes"
        :key="brush"
        type="button"
        :aria-pressed="mode === brush"
        @click="mode = mode === brush ? null : brush"
      >{{ t(`studio.map.brush.${brush}`) }}</button>
      <label>
        {{ t('studio.map.brush.radius') }}
        <input
          v-model.number="radius" type="range" min="64" max="1024" step="64"
          :aria-valuetext="t('studio.map.brush.meters', { value: radius })"
        />
        <span aria-hidden="true">{{ t('studio.map.brush.meters', { value: radius }) }}</span>
      </label>
      <label>
        {{ t('studio.map.brush.strength') }}
        <input
          v-model.number="strength" type="range" min="8" max="256" step="8"
          :aria-valuetext="t('studio.map.brush.meters', { value: strength })"
        />
        <span aria-hidden="true">{{ t('studio.map.brush.meters', { value: strength }) }}</span>
      </label>
      <button type="button" :disabled="busy || !canUndo" @click="emit('undo')">{{ t('studio.map.brush.undo') }}</button>
      <button type="button" :disabled="busy || !canUndo" @click="emit('export')">{{ t('studio.map.brush.export') }}</button>
    </div>
    <div class="brush-status" role="status">{{ status || (mode ? t('studio.map.brush.hint') : '') }}</div>
  </div>
</template>

<style scoped>
.terrain-tools {
  position: absolute;
  z-index: 2;
  inset-inline-start: 50%;
  inset-block-end: 30px;
  translate: -50% 0;
  inline-size: max-content;
  max-inline-size: calc(100% - 24px);
  padding: 6px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--foreground);
  box-shadow: 0 4px 14px rgb(0 0 0 / 30%);
  font-size: 12px;
}
.brush-row, label { display: flex; align-items: center; gap: 6px; }
.brush-row { flex-wrap: wrap; justify-content: center; }
button {
  min-block-size: 28px;
  padding-inline: 7px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--foreground);
}
button[aria-pressed="true"] { background: var(--primary); color: var(--primary-foreground); }
button:focus-visible, input:focus-visible { outline: 2px solid var(--primary); outline-offset: 2px; }
input { inline-size: 66px; block-size: 28px; accent-color: var(--primary); }
label span { min-inline-size: 5ch; font-variant-numeric: tabular-nums; }
button:disabled { opacity: .5; }
.brush-status { text-align: center; }
.brush-status:not(:empty) { padding: 4px 2px 0; overflow-wrap: anywhere; }
</style>
