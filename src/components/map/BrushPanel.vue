<script setup lang="ts">
/**
 * 画刷编辑浮层（P1-5 UI 重构）：固定在预览器右侧的浮动面板。
 * 画刷清单选择 / 添加模式 / stamp 增删撤销 / 保存为 overlay 包。
 * 状态全部由父级持有——地图面板卡片与全屏检查 sheet 两处复用。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FCheckbox from "@/components/ui/FCheckbox.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import type { BrushList } from "@/lib/region-map";

const props = defineProps<{
  brushLists: BrushList[];
  selectedInstance: string;
  placementMode: boolean;
  pendingAdds: [number, number][];
  pendingRemoves: number[];
  busy: boolean;
}>();

const emit = defineEmits<{
  (e: "select", instance: string): void;
  (e: "update:placementMode", value: boolean): void;
  (e: "toggle-remove", index: number): void;
  (e: "undo-add", index: number): void;
  (e: "save"): void;
  (e: "close"): void;
}>();

const { t } = useI18n();

const selected = computed(
  () => props.brushLists.find((b) => b.instance === props.selectedInstance) ?? null,
);
</script>

<template>
  <aside class="brush-float" :aria-label="t('studio.map.brushEditorTitle')">
    <header class="brush-float-header">
      <span class="brush-float-title">
        <FIcon name="Brush" :size="14" aria-label="" />
        {{ t("studio.map.brushEditorTitle") }}
      </span>
      <button
        type="button"
        class="brush-float-close"
        :aria-label="t('shell.close')"
        @click="emit('close')"
      >
        <FIcon name="X" :size="13" aria-label="" />
      </button>
    </header>
    <div class="brush-float-body">
      <button
        v-for="b in brushLists"
        :key="b.instance"
        type="button"
        class="brush-row"
        :class="{ active: selectedInstance === b.instance }"
        @click="emit('select', b.instance)"
      >
        <FIcon
          :name="selectedInstance === b.instance ? 'Check' : 'Square'"
          :size="13"
          aria-label=""
        />
        {{ b.name }} · {{ b.stamps.length }}
      </button>
      <p v-if="!brushLists.length" class="brush-float-hint">
        {{ t("studio.map.emptySide") }}
      </p>
      <template v-if="selected">
        <label class="brush-toggle">
          <FCheckbox
            :model-value="placementMode"
            @update:model-value="emit('update:placementMode', $event)"
          />
          {{ t("studio.map.brushAddMode") }}
        </label>
        <p v-if="placementMode" class="brush-float-hint accent">
          {{ t("studio.map.brushAddHint") }}
        </p>
        <ul class="stamp-list">
          <li
            v-for="(stamp, i) in selected.stamps"
            :key="`s${i}`"
            :class="{ removed: pendingRemoves.includes(i) }"
          >
            <span class="mono">{{ stamp[0].toFixed(0) }}, {{ stamp[1].toFixed(0) }}</span>
            <button type="button" class="stamp-btn" @click="emit('toggle-remove', i)">
              {{ t("studio.map.brushRemove") }}
            </button>
          </li>
          <li v-for="(stamp, i) in pendingAdds" :key="`a${i}`" class="pending">
            <span class="mono">{{ stamp[0] }}, {{ stamp[1] }}</span>
            <button type="button" class="stamp-btn" @click="emit('undo-add', i)">
              {{ t("studio.map.stampUndo") }}
            </button>
          </li>
        </ul>
        <button
          type="button"
          class="brush-float-save"
          :disabled="busy || (!pendingAdds.length && !pendingRemoves.length)"
          @click="emit('save')"
        >
          {{ t("studio.map.brushSave") }}
        </button>
      </template>
      <p v-else class="brush-float-hint">{{ t("studio.map.brushNone") }}</p>
    </div>
  </aside>
</template>

<style scoped>
.brush-float {
  background: color-mix(in srgb, var(--surface) 92%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md, 0 8px 24px rgb(0 0 0 / 0.35));
  backdrop-filter: blur(6px);
  display: flex;
  flex-direction: column;
  max-height: 100%;
  width: 248px;
}
.brush-float-header {
  align-items: center;
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 0.5rem;
  justify-content: space-between;
  padding: 8px 10px;
}
.brush-float-title {
  align-items: center;
  color: var(--foreground);
  display: inline-flex;
  font-size: 0.8125rem;
  font-weight: 600;
  gap: 6px;
}
.brush-float-close {
  background: transparent;
  border: none;
  color: var(--muted-foreground);
  cursor: pointer;
  padding: 2px;
}
.brush-float-close:hover {
  color: var(--foreground);
}
.brush-float-body {
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow-y: auto;
  padding: 8px 10px 10px;
}
.brush-row {
  align-items: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font-size: 0.75rem;
  gap: 6px;
  padding: 4px 6px;
  text-align: left;
}
.brush-row:hover {
  color: var(--foreground);
}
.brush-row.active {
  background: var(--surface-hover, color-mix(in srgb, var(--border) 30%, transparent));
  border-color: var(--border);
  color: var(--foreground);
}
.brush-toggle {
  align-items: center;
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font-size: 0.75rem;
  gap: 8px;
  margin-top: 4px;
}
.brush-float-hint {
  color: var(--muted-foreground);
  font-size: 0.7rem;
  margin: 2px 0;
}
.brush-float-hint.accent {
  color: var(--primary, var(--foreground));
}
.brush-float-empty {
  color: var(--muted-foreground);
  font-size: 0.75rem;
}
.stamp-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  list-style: none;
  margin: 4px 0;
  max-height: 220px;
  overflow-y: auto;
  padding: 0;
}
.stamp-list li {
  align-items: center;
  display: flex;
  font-size: 0.7rem;
  gap: 0.5rem;
  justify-content: space-between;
}
.stamp-list li.removed {
  opacity: 0.45;
  text-decoration: line-through;
}
.stamp-list li.pending {
  color: var(--primary, var(--foreground));
}
.stamp-btn {
  background: transparent;
  border: none;
  color: var(--muted-foreground);
  cursor: pointer;
  font-size: 0.7rem;
  padding: 2px 4px;
}
.stamp-btn:hover {
  color: var(--foreground);
}
.brush-float-save {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  font-size: 0.75rem;
  margin-top: 4px;
  padding: 6px 10px;
}
.brush-float-save:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.brush-float-save:hover:not(:disabled) {
  border-color: var(--border-strong, var(--border));
}
.mono {
  font-family: var(--font-mono, ui-monospace, monospace);
}
</style>
