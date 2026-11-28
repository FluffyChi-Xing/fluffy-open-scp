<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSelect from "@/components/ui/FSelect.vue";
import type { LotUnitDto } from "@/api/tauri";
import SpawnerLibraryPanel from "@/pages/packages/components/unit-inspector/SpawnerLibraryPanel.vue";
import { PERFORMANCE_PRESETS, performancePreset } from "./performanceBindings";
const props = defineProps<{
  units: readonly LotUnitDto[];
  editEnabled: boolean;
  selectedUnitId: string | null;
}>();
const emit = defineEmits<{
  place: [selection: { performancePreset?: string }];
  select: [id: string];
}>();
const { t } = useI18n();
const selected = ref("");
const preset = computed(() => performancePreset(selected.value));
const hex = (id: number) =>
  `0x${id.toString(16).toUpperCase().padStart(8, "0")}`;
const options = computed(() => [
  { value: "", label: t("spawnerPanel.performanceAutomatic") },
  ...PERFORMANCE_PRESETS.map((p) => ({
    value: p.id,
    label: `${t(`spawnerPanel.performanceCategories.${p.category}`)} · ${hex(p.vignetteId[2])} → ${hex(p.definition[2])}`,
  })),
]);
function place() {
  if (props.editEnabled)
    emit("place", selected.value ? { performancePreset: selected.value } : {});
}
</script>
<template>
  <SpawnerLibraryPanel
    :units="units"
    :edit-enabled="editEnabled"
    :selected-unit-id="selectedUnitId"
    @select="emit('select', $event)"
  >
    <template #preset>
      <div class="performance-create">
        <button
          type="button"
          class="anchor-preset"
          :disabled="!editEnabled"
          :title="t('spawnerPanel.presetStandardHint')"
          @pointerdown.stop.prevent="place"
          @keydown.enter.prevent="place"
          @keydown.space.prevent="place"
        >
          <FIcon name="MapPin" :size="16" aria-label="" />
          <span>{{ t("spawnerPanel.presetStandard") }}</span>
        </button>
        <label class="performance-select" for="performance-preset">
          <span>{{ t("spawnerPanel.performanceSelect") }}</span>
          <FSelect
            id="performance-preset"
            v-model="selected"
            :options="options"
            :disabled="!editEnabled"
          />
        </label>
      </div>
      <p v-if="preset" class="performance-hint">
        {{
          t(
            preset.anchors.length
              ? "spawnerPanel.performanceAnchors"
              : "spawnerPanel.performanceNoAnchor",
            { count: preset.anchors.length },
          )
        }}
      </p>
      <p v-if="preset?.category.startsWith('industrial')" class="performance-hint">
        {{ t('spawnerPanel.performanceIndustrial3Unavailable') }}
      </p>
    </template>
  </SpawnerLibraryPanel>
</template>
<style scoped>
.performance-create {
  display: flex;
  align-items: stretch;
  gap: 8px;
  min-width: 0;
}
.anchor-preset {
  width: 88px;
  flex: none;
  min-height: 58px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  background: var(--surface-elevated);
  cursor: grab;
  font: inherit;
  font-size: 11px;
}
.anchor-preset:hover:not(:disabled) {
  color: var(--primary);
  border-color: var(--primary);
}
.anchor-preset:focus-visible,
:deep(.f-select:focus-visible) {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}
.anchor-preset:disabled,
:deep(.f-select:disabled) {
  opacity: 0.5;
  cursor: not-allowed;
}
.performance-select {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 5px;
  color: var(--muted-foreground);
  font-size: 11px;
}
.performance-select :deep(.f-select) {
  width: 100%;
  min-width: 0;
  min-height: 29px;
  background: var(--surface-elevated);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  font: inherit;
}
.performance-hint {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.5;
}
</style>
