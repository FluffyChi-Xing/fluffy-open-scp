<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { LotUnitDto } from "@/api/tauri";
import {
  ANCHOR_KIND_LABEL,
  CAMERA_ANCHOR_ID,
  UI_ANCHOR_ID,
  describeSpawners,
  summarizeSpawners,
} from "./spawnerAnchor";

/**
 * 组件库「生成器」页签（重做版）。
 *
 * 旧版只有一个可拖动的占位卡片，用户既看不到本 Lot 有哪些锚点，也不知道
 * 锚点意味着什么。新版把三件事放在一起：
 *  ① 三种预设（标准 / UI 气泡 0x141EED53 / 镜头 0x9901C8F9）——拖入即放置；
 *  ② 本 Lot 锚点清单——点选即在视口选中（带语义徽章与重名提示）；
 *  ③ 统计行——未命名与重名数量直接暴露写入风险。
 */
const props = withDefaults(
  defineProps<{
    units: readonly LotUnitDto[];
    editEnabled?: boolean;
    selectedUnitId?: string | null;
  }>(),
  { editEnabled: false, selectedUnitId: null },
);
const emit = defineEmits<{
  place: [spawnerId: number | null];
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

const PRESETS: { id: number | null; icon: string; label: string; hint: string }[] =
  [
    {
      id: null,
      icon: "MapPin",
      label: "spawnerPanel.presetStandard",
      hint: "spawnerPanel.presetStandardHint",
    },
    {
      id: UI_ANCHOR_ID,
      icon: "Info",
      label: "spawnerPanel.presetUi",
      hint: "spawnerPanel.presetUiHint",
    },
    {
      id: CAMERA_ANCHOR_ID,
      icon: "Camera",
      label: "spawnerPanel.presetCamera",
      hint: "spawnerPanel.presetCameraHint",
    },
  ];

function beginDrag(event: PointerEvent, spawnerId: number | null) {
  if (!props.editEnabled) return;
  event.preventDefault();
  emit("place", spawnerId);
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

    <div class="sl-presets">
      <button
        v-for="preset in PRESETS"
        :key="String(preset.id)"
        type="button"
        class="sl-preset"
        :title="t(preset.hint)"
        :disabled="!editEnabled"
        @pointerdown.stop.prevent="beginDrag($event, preset.id)"
      >
        <FIcon :name="preset.icon" :size="16" aria-label="" />
        <span>{{ t(preset.label) }}</span>
      </button>
    </div>

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
.sl-presets {
  display: grid;
  gap: 5px;
  grid-template-columns: repeat(3, 1fr);
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
