<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { TopicGroup } from "../notes-model";

/**
 * 分类导航栏（2026-10-28 重构）：统计图与导航合一——每行内嵌一条
 * 长度∝批注数的微型条（lieflat Tick Rows 语法：实心、明度即数据、
 * 不断轴），排名即排序。取代独立的 TopicBars 卡片：图表信息变成
 * 常驻可见的结构，不再占用首屏独立版面。支持 ↑↓/Home/End 键盘换选。
 */
const props = defineProps<{
  groups: TopicGroup[];
  selected: string | null;
}>();

const emit = defineEmits<{ select: [topic: string] }>();
const { t } = useI18n();

const listRef = ref<HTMLUListElement | null>(null);

const maxCount = computed(() =>
  Math.max(1, ...props.groups.map((group) => group.items.length)),
);
function barWidth(group: TopicGroup): number {
  return (group.items.length / maxCount.value) * 100;
}

/** 键盘换选：↑↓ 移动、Home/End 首尾；焦点跟随选中项。 */
function onKeydown(event: KeyboardEvent) {
  if (!props.groups.length) return;
  const current = props.groups.findIndex(
    (group) => group.topic === props.selected,
  );
  const lastIndex = props.groups.length - 1;
  const next =
    event.key === "ArrowDown"
      ? Math.min(lastIndex, current + 1)
      : event.key === "ArrowUp"
        ? Math.max(0, current - 1)
        : event.key === "Home"
          ? 0
          : event.key === "End"
            ? lastIndex
            : -1;
  if (next < 0) return;
  event.preventDefault();
  emit("select", props.groups[next].topic);
  const buttons =
    listRef.value?.querySelectorAll<HTMLButtonElement>(".rail-row");
  buttons?.[next]?.focus();
}
</script>

<template>
  <nav class="rail" :aria-label="t('notes.railLabel')">
    <ul ref="listRef" class="rail-list" @keydown="onKeydown">
      <li v-for="group in groups" :key="group.topic">
        <button
          type="button"
          class="rail-row"
          :class="{ active: group.topic === selected }"
          :style="{ '--bar': `${barWidth(group)}%` }"
          :aria-current="group.topic === selected"
          :title="group.topic"
          @click="emit('select', group.topic)"
        >
          <span class="rail-label">{{ group.topic }}</span>
          <span class="rail-count">{{ group.items.length }}</span>
        </button>
      </li>
    </ul>
  </nav>
</template>

<style scoped>
.rail {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow-y: auto;
  overscroll-behavior: contain;
}
.rail-list {
  display: grid;
  list-style: none;
  margin: 0;
  padding: 5px;
}
.rail-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 8px;
  justify-content: space-between;
  min-height: 29px;
  padding: 4px 9px;
  position: relative;
  text-align: start;
  width: 100%;
}
/* 行内迷你条：长度∝批注数（排名语义），垫在文字下方 */
.rail-row::before {
  background: color-mix(in srgb, var(--foreground) 9%, transparent);
  border-radius: 4px;
  content: "";
  inset: 4px auto 4px 0;
  position: absolute;
  transition: width 360ms cubic-bezier(0.22, 1, 0.36, 1);
  width: var(--bar);
}
.rail-row:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.rail-row.active {
  background: var(--accent);
  color: var(--foreground);
}
.rail-row.active::before {
  background: color-mix(in srgb, var(--primary) 26%, transparent);
}
.rail-label {
  min-width: 0;
  overflow: hidden;
  position: relative;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rail-row.active .rail-label {
  font-weight: 650;
}
.rail-count {
  flex: none;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  position: relative;
}
.rail-row:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: -2px;
}
@media (prefers-reduced-motion: reduce) {
  .rail-row::before {
    transition: none;
  }
}
</style>
