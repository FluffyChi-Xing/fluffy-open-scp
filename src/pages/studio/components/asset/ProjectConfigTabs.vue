<script setup lang="ts" generic="Tab extends string">
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";

defineProps<{
  tabs: readonly { id: Tab; icon: string; labelKey: string }[];
  label: string;
}>();
const selected = defineModel<Tab>({ required: true });
const { t } = useI18n();
</script>

<template>
  <nav
    class="project-tabs"
    role="tablist"
    aria-orientation="vertical"
    :aria-label="label"
  >
    <button
      v-for="tab in tabs"
      :key="tab.id"
      type="button"
      role="tab"
      :aria-selected="selected === tab.id"
      :class="{ active: selected === tab.id }"
      @click="selected = tab.id"
    >
      <FIcon :name="tab.icon" :size="13" aria-label="" />
      {{ t(tab.labelKey) }}
    </button>
  </nav>
</template>

<style scoped>
.project-tabs {
  border-inline-end: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-inline-size: 168px;
  padding: 10px 8px;
  overflow-y: auto;
}
.project-tabs button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 7px;
  min-height: 32px;
  padding: 0 9px;
  text-align: start;
}
.project-tabs button:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.project-tabs button.active {
  background: color-mix(in srgb, var(--primary) 14%, transparent);
  color: var(--primary);
  font-weight: 600;
}
</style>
