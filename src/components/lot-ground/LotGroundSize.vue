<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { LOT_METERS_PER_PIXEL, maskSizeForLot } from "@/lib/lot-ground/ground";

const props = defineProps<{
  size: readonly [number, number] | null;
  disabled?: boolean;
  metersPerPixel?: number;
}>();
const emit = defineEmits<{ apply: [size: [number, number]] }>();
const { t } = useI18n();

const width = ref(48);
const depth = ref(48);
watch(
  () => props.size,
  (size) => {
    if (!size) return;
    width.value = size[0];
    depth.value = size[1];
  },
  { immediate: true },
);

const mpp = computed(() => props.metersPerPixel ?? LOT_METERS_PER_PIXEL);
const valid = computed(() => width.value > 0 && depth.value > 0);
const pixels = computed(() => maskSizeForLot([width.value || 0, depth.value || 0], mpp.value));
const changed = computed(
  () => !props.size || props.size[0] !== width.value || props.size[1] !== depth.value,
);
</script>

<template>
  <section class="lot-size">
    <h3>{{ t("rasterEditor.ground.size") }}</h3>
    <div class="inputs">
      <label>
        <span>{{ t("rasterEditor.ground.width") }}</span>
        <input v-model.number="width" type="number" min="1" step="1" :disabled="disabled" />
      </label>
      <span aria-hidden="true">×</span>
      <label>
        <span>{{ t("rasterEditor.ground.depth") }}</span>
        <input v-model.number="depth" type="number" min="1" step="1" :disabled="disabled" />
      </label>
      <button
        type="button"
        :disabled="disabled || !valid || !changed"
        @click="emit('apply', [width, depth])"
      >
        {{ t("rasterEditor.ground.applySize") }}
      </button>
    </div>
    <p class="hint">
      {{ t("rasterEditor.ground.sizeDerived", { w: pixels.width, h: pixels.height, mpp }) }}
    </p>
  </section>
</template>

<style scoped>
.lot-size { display: grid; gap: 6px; }
h3 { font-size: 12px; font-weight: 600; margin: 0; }
.inputs { align-items: end; display: flex; gap: 6px; }
label { display: grid; flex: 1; font-size: 11px; gap: 3px; color: var(--muted-foreground); }
input {
  background: var(--background);
  border: 1px solid var(--border);
  border-radius: 6px;
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--foreground) 6%, transparent);
  color: var(--foreground);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  min-height: 30px;
  min-width: 0;
  padding: 5px 8px;
  width: 100%;
}
input:hover:not(:disabled) { border-color: color-mix(in srgb, var(--primary) 45%, var(--border)); }
input:focus-visible {
  border-color: var(--primary);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--primary) 28%, transparent);
  outline: none;
}
input:disabled { opacity: 0.55; }
button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--foreground);
  font-size: 11px;
  min-height: 30px;
  padding: 5px 10px;
  white-space: nowrap;
}
button:disabled { opacity: 0.5; }
.hint { color: var(--muted-foreground); font-size: 11px; margin: 0; }
</style>
