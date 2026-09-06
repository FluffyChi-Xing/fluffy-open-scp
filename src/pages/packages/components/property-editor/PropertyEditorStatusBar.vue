<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { LotUnitDto } from "@/api/tauri";
import {
  unitLabel,
  type ModelState,
  type UnitGrouping,
} from "./usePropertyEditorSession";

const props = defineProps<{
  grouping: UnitGrouping;
  modelState: ModelState;
  selectedUnit: LotUnitDto | null;
}>();
const { t } = useI18n();

const counts = computed(() => [
  { label: t("package.groupLights"), value: props.grouping.lights.length },
  { label: t("package.groupDecals"), value: props.grouping.decals.length },
  { label: t("package.groupProps"), value: props.grouping.props.length },
  { label: t("package.groupEffects"), value: props.grouping.effects.length },
  { label: t("package.groupSpawners"), value: props.grouping.spawners.length },
  { label: t("package.groupPaths"), value: props.grouping.pathPoints.length },
]);
const modelLabel = computed(() => {
  switch (props.modelState) {
    case "ready":
      return t("package.modelStateReady");
    case "missing":
      return t("package.modelStateMissing");
    case "error":
      return t("package.modelStateError");
    case "loading":
      return t("common.loading");
    default:
      return t("package.modelStateMissing");
  }
});
</script>

<template>
  <footer class="status-bar">
    <span class="status-selection">
      {{
        selectedUnit
          ? unitLabel(selectedUnit, t)
          : $t("package.statusNoSelection")
      }}
    </span>
    <span class="status-divider" aria-hidden="true">·</span>
    <span>{{ modelLabel }}</span>
    <span class="status-divider" aria-hidden="true">·</span>
    <span v-for="count in counts" :key="count.label" class="status-count">
      {{ count.label }} {{ count.value }}
    </span>
  </footer>
</template>

<style scoped>
.status-bar {
  align-items: center;
  border-top: 1px solid var(--border);
  color: var(--muted-foreground);
  display: flex;
  flex-shrink: 0;
  flex-wrap: nowrap;
  font-size: 11px;
  gap: 8px;
  height: 30px;
  overflow: hidden;
  padding: 0 16px;
  white-space: nowrap;
}
.status-selection {
  color: var(--foreground);
  font-weight: 600;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}
.status-divider {
  color: var(--subtle-foreground);
}
.status-count {
  white-space: nowrap;
}
</style>
