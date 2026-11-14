<script setup lang="ts">
import FIcon from "@/components/extensions/FIcon.vue";
import { tabTitle, type EditorTab } from "../editor-tabs";

/**
 * 文档页签条（仿 Obsidian）：横向排列已打开文档，活动页签高亮，
 * 中键或 × 关闭；右侧 actions 插槽放模式切换/保存等针对活动页签的动作。
 */
const props = defineProps<{
  tabs: readonly EditorTab[];
  activePath: string;
}>();
const emit = defineEmits<{
  activate: [path: string];
  close: [path: string];
}>();
</script>

<template>
  <div class="doc-tabs">
    <div
      class="tab-strip"
      role="tablist"
      :aria-label="$t('workspace.tabsLabel')"
    >
      <div
        v-for="tab in props.tabs"
        :key="tab.path"
        class="doc-tab"
        :class="{ active: tab.path === props.activePath }"
      >
        <button
          class="tab-main"
          role="tab"
          type="button"
          :aria-selected="tab.path === props.activePath"
          :title="tab.path"
          @click="emit('activate', tab.path)"
          @auxclick.middle.prevent="emit('close', tab.path)"
        >
          <span
            v-if="tab.dirty"
            class="dirty-dot"
            :title="$t('workspace.dirty')"
          /><FIcon
            v-if="tab.loading"
            name="Loader2"
            :size="12"
            class="tab-spinner"
            aria-label=""
          /><span class="tab-title">{{ tabTitle(tab.path) }}</span>
        </button>
        <button
          class="tab-close"
          type="button"
          :aria-label="`${tabTitle(tab.path)} ${$t('workspace.tabClose')}`"
          @click="emit('close', tab.path)"
        >
          <FIcon name="X" :size="12" aria-label="" />
        </button>
      </div>
    </div>
    <div class="tab-actions">
      <slot name="actions" />
    </div>
  </div>
</template>

<style scoped>
.doc-tabs {
  align-items: stretch;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  min-height: 34px;
  min-width: 0;
}
.tab-strip {
  display: flex;
  flex: 1;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: thin;
}
.doc-tab {
  align-items: stretch;
  border-right: 1px solid var(--border);
  display: flex;
  flex: none;
  max-width: 220px;
  min-width: 0;
  transition: background-color 140ms ease;
}
.doc-tab.active {
  background: var(--surface-elevated);
  box-shadow: inset 0 2px 0 var(--primary);
}
.tab-main {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 6px;
  min-height: 34px;
  min-width: 0;
  padding: 0 4px 0 12px;
  text-align: start;
}
/* hover 统一落在整条页签上（标题区/关闭钮同色），活动页签 hover 不变色，
   避免 .tab-main 单独变色造成的两段割裂观感 */
.doc-tab:hover {
  background: var(--surface-hover);
}
.doc-tab:hover .tab-main {
  color: var(--foreground);
}
.doc-tab.active,
.doc-tab.active:hover {
  background: var(--surface-elevated);
}
.doc-tab.active .tab-main {
  color: var(--foreground);
  font-weight: 650;
}
.tab-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dirty-dot {
  background: var(--primary);
  border-radius: 50%;
  flex: none;
  height: 6px;
  width: 6px;
}
.tab-spinner {
  animation: tab-spin 900ms linear infinite;
  color: var(--muted-foreground);
  flex: none;
}
@keyframes tab-spin {
  to {
    transform: rotate(360deg);
  }
}
@media (prefers-reduced-motion: reduce) {
  .tab-spinner {
    animation: none;
  }
}
.tab-close {
  align-items: center;
  align-self: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--subtle-foreground);
  cursor: pointer;
  display: inline-flex;
  flex: none;
  height: 20px;
  justify-content: center;
  margin-inline-end: 6px;
  padding: 0;
  transition:
    background-color 140ms ease,
    color 140ms ease;
  width: 20px;
}
/* 中性叠加色：在 hover 层与活动 elevated 层上都可辨识 */
.tab-close:hover {
  background: color-mix(in srgb, var(--foreground) 14%, transparent);
  color: var(--foreground);
}
.tab-actions {
  align-items: center;
  display: flex;
  flex: none;
  gap: 6px;
  padding: 0 10px;
}
</style>
