<script setup lang="ts">
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import { computed } from "vue";
import {
  RASTER_PROFILES,
  type RasterProfile,
} from "@/lib/raster-editor/schema";
defineProps<{ modelValue: RasterProfile; locked?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [value: RasterProfile] }>();
const groups = computed(() => [
  t("rasterEditor.text080"),
  t("rasterEditor.text081"),
  "Decal",
  t("rasterEditor.text082"),
]);
</script>
<template>
  <nav class="raster-profiles" :aria-label="t('rasterEditor.text083')">
    <section v-for="group in groups" :key="group">
      <h3>{{ group }}</h3>
      <button
        v-for="item in RASTER_PROFILES.filter((p) => p.group === group)"
        :key="item.id"
        :disabled="locked"
        :aria-pressed="modelValue === item.id"
        :title="item.hint"
        @click="emit('update:modelValue', item.id)"
      >
        <code v-if="item.id.startsWith('slot')">{{ item.id.slice(4) }}</code
        >{{ item.label }}
      </button>
    </section>
  </nav>
</template>
<style scoped>
.raster-profiles {
  display: grid;
  align-content: start;
  gap: 20px;
  padding: 16px 10px;
}
h3 {
  margin: 0 8px 8px;
  color: var(--muted-foreground);
  font-size: 11px;
}
button {
  width: 100%;
  display: flex;
  gap: 10px;
  align-items: center;
  padding: 8px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--muted-foreground);
  text-align: start;
  border-radius: 5px;
  font-size: 12px;
  cursor: pointer;
}
button[aria-pressed="true"] {
  color: var(--primary);
  border-color: var(--border);
  background: var(--surface-hover);
}
code {
  width: 18px;
  text-align: center;
  opacity: 0.8;
}
button:focus-visible {
  outline: 2px solid var(--primary);
}
@media (max-width: 700px) {
  .raster-profiles {
    grid-template-columns: repeat(2, 1fr);
    gap: 12px;
  }
}
</style>
