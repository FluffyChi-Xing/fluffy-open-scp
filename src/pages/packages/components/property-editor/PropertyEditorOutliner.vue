<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { LotUnitDto } from "@/api/tauri";
import { unitLabel } from "./usePropertyEditorSession";
import type { UnitGrouping } from "./usePropertyEditorSession";
import { unitId } from "./unitGizmos";

const props = defineProps<{
  grouping: UnitGrouping;
  selectedId: string | null;
  hiddenUnits: Set<string>;
  groupVisibility: Record<string, boolean>;
}>();
defineEmits<{
  select: [id: string | null];
  "toggle-unit": [unit: LotUnitDto];
  "toggle-group": [name: string];
}>();
const { t } = useI18n();

interface OutlinerSection {
  key: string;
  label: string;
  items: LotUnitDto[];
}

const sections = computed<OutlinerSection[]>(() => [
  {
    key: "lights",
    label: t("package.groupLights"),
    items: props.grouping.lights,
  },
  {
    key: "decals",
    label: t("package.groupDecals"),
    items: props.grouping.decals,
  },
  { key: "props", label: t("package.groupProps"), items: props.grouping.props },
  {
    key: "effects",
    label: t("package.groupEffects"),
    items: props.grouping.effects,
  },
  {
    key: "spawners",
    label: t("package.groupSpawners"),
    items: props.grouping.spawners,
  },
  {
    key: "paths",
    label: t("package.groupPaths"),
    items: props.grouping.pathPoints,
  },
]);
</script>

<template>
  <aside class="outliner" :aria-label="$t('package.outliner')">
    <div
      v-for="section in sections"
      :key="section.key"
      class="outliner-section"
    >
      <div class="outliner-section-head">
        <button
          type="button"
          class="outliner-eye"
          :aria-pressed="groupVisibility[section.key] !== false"
          :aria-label="
            $t('package.visibilityToggleFor', { name: section.label })
          "
          @click="$emit('toggle-group', section.key)"
        >
          <FIcon
            :name="groupVisibility[section.key] === false ? 'EyeOff' : 'Eye'"
            :size="13"
            aria-label=""
          />
        </button>
        <span class="outliner-section-title">{{ section.label }}</span>
        <small>{{ section.items.length }}</small>
      </div>
      <ul>
        <li
          v-for="unit in section.items"
          :key="unitId(unit)"
          :class="{ selected: selectedId === unitId(unit), hidden: hiddenUnits.has(unitId(unit)) }"
        >
          <button
            type="button"
            class="outliner-eye"
            :aria-pressed="!hiddenUnits.has(unitId(unit))"
            :aria-label="$t('package.visibilityToggleFor', { name: unitLabel(unit, t) })"
            @click="$emit('toggle-unit', unit)"
          >
            <FIcon
              :name="hiddenUnits.has(unitId(unit)) ? 'EyeOff' : 'Eye'"
              :size="12"
              aria-label=""
            />
          </button>
          <button type="button" class="outliner-item" @click="$emit('select', unitId(unit))">
            <span class="outliner-kind-dot" :data-kind="unit.kind" aria-hidden="true" />
            {{ unitLabel(unit, t) }}
          </button>
        </li>
        <li v-if="!section.items.length" class="outliner-empty">—</li>
      </ul>
    </div>
  </aside>
</template>

<style scoped>
.outliner {
  border-inline-end: 1px solid var(--border);
  overflow-y: auto;
  min-height: 0;
  padding: 8px;
}
.outliner-section {
  margin-bottom: 10px;
}
.outliner-section-head {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 11px;
  gap: 6px;
  letter-spacing: 0.05em;
  padding: 3px 4px;
  text-transform: uppercase;
}
.outliner-section-head small {
  color: var(--subtle-foreground);
  margin-inline-start: auto;
}
.outliner-eye {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 20px;
  min-width: 20px;
  padding: 0;
}
.outliner-eye:hover {
  color: var(--foreground);
}
.outliner-section-title {
  font-weight: 600;
}
ul {
  display: grid;
  gap: 1px;
  list-style: none;
  margin: 2px 0 0;
  padding: 0;
}
li {
  align-items: center;
  border-radius: var(--radius-sm);
  display: flex;
}
li:hover {
  background: var(--surface-hover);
}
li.selected {
  background: var(--accent);
}
li.hidden .outliner-item {
  color: var(--subtle-foreground);
  text-decoration: line-through;
}
.outliner-item {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12px;
  gap: 7px;
  min-height: 24px;
  overflow: hidden;
  padding: 0 6px;
  text-align: start;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.outliner-kind-dot {
  background: var(--muted-foreground);
  border-radius: 999px;
  flex: none;
  height: 7px;
  width: 7px;
}
.outliner-kind-dot[data-kind="light"] {
  background: #f5c542;
}
.outliner-kind-dot[data-kind="effect"] {
  background: #ffd700;
}
.outliner-kind-dot[data-kind="prop"] {
  background: #c81e1e;
}
.outliner-kind-dot[data-kind="spawner"] {
  background: #1e40c8;
}
.outliner-kind-dot[data-kind="pathPoint"] {
  background: #00c8c8;
}
.outliner-kind-dot[data-kind="decal"] {
  background: #33cc33;
}
.outliner-empty {
  color: var(--subtle-foreground);
  font-size: 11px;
  padding: 2px 26px;
}
</style>
