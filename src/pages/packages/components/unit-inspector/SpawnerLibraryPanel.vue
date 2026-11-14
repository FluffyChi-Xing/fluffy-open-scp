<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { LotUnitDto } from "@/api/tauri";
import {
  ANCHOR_KIND_LABEL,
  describeSpawners,
  summarizeSpawners,
} from "./spawnerAnchor";

const props = withDefaults(
  defineProps<{
    units: readonly LotUnitDto[];
    editEnabled?: boolean;
    selectedUnitId?: string | null;
  }>(),
  { editEnabled: false, selectedUnitId: null },
);
const emit = defineEmits<{
  place: [];
  select: [unitId: string];
}>();

const { t } = useI18n();
const search = ref("");

const anchors = computed(() => describeSpawners(props.units));
const summary = computed(() => summarizeSpawners(props.units));
const filtered = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  if (!keyword) return anchors.value;
  return anchors.value.filter(
    (anchor) =>
      anchor.label.toLowerCase().includes(keyword) ||
      t(ANCHOR_KIND_LABEL[anchor.kind]).toLowerCase().includes(keyword),
  );
});

function beginDrag() {
  if (props.editEnabled) emit("place");
}
</script>

<template>
  <div class="spawner-library">
    <p class="sl-stats">
      {{
        t("spawnerPanel.stats", {
          total: summary.total,
          named: summary.named,
          unnamed: summary.unnamed,
        })
      }}
      <template v-if="summary.duplicates">
        {{
          t("spawnerPanel.statsDuplicate", { count: summary.duplicates })
        }}
      </template>
    </p>

    <button
      type="button"
      class="sl-preset"
      :title="t('spawnerPanel.presetStandardHint')"
      :disabled="!editEnabled"
      @pointerdown.stop.prevent="beginDrag"
    >
      <FIcon name="MapPin" :size="16" aria-label="" />
      <span>{{ t("spawnerPanel.presetStandard") }}</span>
    </button>

    <input
      v-model="search"
      type="search"
      class="sl-search"
      :placeholder="t('spawnerPanel.listSearch')"
      :aria-label="t('spawnerPanel.listSearch')"
    />

    <div class="sl-list" role="listbox" :aria-label="t('spawnerPanel.listTitle')">
      <button
        v-for="anchor in filtered"
        :key="anchor.unitId"
        type="button"
        role="option"
        class="sl-item"
        :class="{ active: anchor.unitId === selectedUnitId }"
        :aria-selected="anchor.unitId === selectedUnitId"
        :title="t('spawnerPanel.selectAnchor')"
        @click="emit('select', anchor.unitId)"
      >
        <span class="sl-item-id">
          {{ anchor.anchorId === null ? t("spawnerPanel.anchorUnnamed") : anchor.label }}
        </span>
        <span class="sl-item-kind" :class="`kind-${anchor.kind}`">
          {{ t(ANCHOR_KIND_LABEL[anchor.kind]) }}
        </span>
        <span v-if="anchor.duplicate" class="sl-item-warn">
          <FIcon name="CircleAlert" :size="11" aria-label="" />
        </span>
      </button>
      <p v-if="!filtered.length" class="sl-empty">
        {{ anchors.length ? t("package.materialsEmpty") : t("spawnerPanel.listEmpty") }}
      </p>
    </div>
  </div>
</template>

<style scoped>
.spawner-library {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}
.sl-stats {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
}
.sl-preset {
  align-items: center;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: grab;
  display: flex;
  flex-direction: column;
  font: inherit;
  font-size: 10px;
  gap: 4px;
  justify-content: center;
  min-height: 58px;
  padding: 7px 3px;
  text-align: center;
}
.sl-preset:hover:not(:disabled) {
  border-color: var(--primary);
  color: var(--primary);
}
.sl-preset:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.sl-search {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 11.5px;
  min-height: 27px;
  padding: 0 9px;
}
.sl-list {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 3px;
  min-height: 0;
  overflow-y: auto;
}
.sl-item {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 11px;
  gap: 6px;
  min-height: 26px;
  padding: 0 8px;
  text-align: start;
}
.sl-item:hover {
  border-color: var(--primary);
}
.sl-item.active {
  border-color: var(--brand);
  color: var(--brand);
}
.sl-item-id {
  font-family: var(--font-mono, monospace);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sl-item-kind {
  color: var(--muted-foreground);
  font-size: 10px;
  margin-inline-start: auto;
  white-space: nowrap;
}
.sl-item-kind.kind-ui {
  color: var(--brand);
}
.sl-item-kind.kind-camera {
  color: #f59e0b;
}
.sl-item-warn {
  color: var(--destructive, #ef4444);
  display: inline-flex;
}
.sl-empty {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
  padding: 10px 4px;
  text-align: center;
}
</style>
