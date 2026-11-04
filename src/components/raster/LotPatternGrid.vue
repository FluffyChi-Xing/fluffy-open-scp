<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{ modelValue: number; disabled?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [value: number] }>();
const { t } = useI18n();
const patterns = Array.from({ length: 16 }, (_, index) => index);
</script>

<template>
  <div class="pattern-grid" role="radiogroup" :aria-label="t('rasterEditor.patternAtlas')">
    <button
      v-for="index in patterns"
      :key="index"
      type="button"
      role="radio"
      :aria-checked="modelValue === index"
      :aria-label="t(`rasterEditor.patternNames.${index}`)"
      :disabled="disabled"
      :title="t(`rasterEditor.patternNames.${index}`)"
      @click="emit('update:modelValue', index)"
    >
      <span class="pattern-cell" :class="`pattern-${index}`" aria-hidden="true" />
      <span>{{ String(index).padStart(2, "0") }}</span>
    </button>
  </div>
</template>

<style scoped>
.pattern-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 4px; margin-top: 4px; }
.pattern-grid button { align-items: center; background: var(--surface-elevated); border: 1px solid var(--border); border-radius: 4px; color: var(--muted-foreground); display: flex; flex-direction: column; font-size: 10px; gap: 2px; min-height: 38px; padding: 3px; }
.pattern-grid button[aria-checked="true"] { border-color: var(--primary); color: var(--foreground); box-shadow: 0 0 0 1px var(--primary); }
.pattern-grid button:disabled { opacity: 0.55; }
.pattern-cell { background: repeating-linear-gradient(45deg, #8d8d8d 0 3px, #cfcfcf 3px 6px); border-radius: 2px; display: block; height: 18px; width: 100%; }
.pattern-1, .pattern-2, .pattern-3, .pattern-9 { background: repeating-linear-gradient(0deg, #777 0 2px, #bbb 2px 5px); }
.pattern-4, .pattern-7 { background: repeating-linear-gradient(135deg, #454545 0 2px, #858585 2px 4px); }
.pattern-5, .pattern-8, .pattern-12, .pattern-13, .pattern-14, .pattern-15 { background: radial-gradient(circle, #557044 0 2px, #9aa879 2px 4px, #526544 4px 6px); }
.pattern-6, .pattern-11 { background: radial-gradient(circle, #76583d 0 2px, #b39775 2px 4px, #664a32 4px 6px); }
.pattern-10 { background: linear-gradient(135deg, #6c95aa, #b7d1d7); }
</style>
