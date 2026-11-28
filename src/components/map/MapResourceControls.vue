<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { resourceColor } from "@/lib/region-map";
import FButton from "@/components/ui/FButton.vue";
import FSlider from "@/components/ui/FSlider.vue";

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
      <FButton
        class="resource-switch"
        variant="ghost"
        size="sm"
        :disabled="disabled"
        :aria-pressed="selected === null"
        @click="selected = null"
      >
        {{ t("studio.map.layerOff") }}
      </FButton>
      <FButton
        v-for="option in options"
        :key="option.kind"
        class="resource-switch"
        variant="ghost"
        size="sm"
        :disabled="disabled || !option.available"
        :aria-pressed="selected === option.kind"
        :title="
          option.available ? option.label : t('studio.map.resourceUnavailable')
        "
        @click="selected = selected === option.kind ? null : option.kind"
      >
        {{ option.label }}
      </FButton>
    </div>
    <div v-if="selected === 'wind'" class="resource-legend">{{ t("mapResources.windLegend") }}</div>
    <div v-else-if="selected" class="resource-legend">
      <span>{{ t("studio.map.resourceLow") }}</span>
      <span
        class="resource-ramp"
        :style="{
          background: `linear-gradient(to right, #e5e5e5, ${resourceColor(selected)})`,
        }"
      ></span>
      <span>{{ t("studio.map.resourceHigh") }}</span>
      <div class="opacity">
        <span>{{ t("studio.map.resourceOpacity") }}</span>
        <FSlider v-model="opacity" class="opacity-slider" :label="t('studio.map.resourceOpacity')" :min="0" :max="1" :step="0.05" />
      </div>
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
.resource-switch {
  min-height: 24px;
  padding-inline: 6px;
  font-weight: 500;
}
.resource-switch[aria-pressed="true"] {
  border-color: var(--border);
  background: var(--surface-elevated);
  color: var(--foreground);
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
.opacity {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-inline-start: 12px;
}
.opacity-slider {
  width: 90px;
}
</style>
