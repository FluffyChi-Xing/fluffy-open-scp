<script setup lang="ts">
/**
 * 上班时间排班拖动条（2026-11-07）。
 *
 * 24 小时时间轴 × 8 个班位（每班 3 小时）。两个拖动点分别标注
 * 「上班」「交班」时刻；拖动后按窗口重排 8 个班位的规模值
 * （窗口内保留原人数，新纳入班位取 6/7 基准值，窗口外 = 0 关班）。
 * 数据契约：schedule[i] 对应 unit 排班列 0x0D00853A + i。
 */
import { computed, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  DAY_HOURS,
  SLOT_HOURS,
  applyShiftWindow,
  deriveShiftWindow,
  moveShiftHandle,
  normalizeSchedule,
  type ShiftHandle,
} from "./shiftSchedule";

const props = defineProps<{
  schedule: number[];
  disabled?: boolean;
}>();

const emit = defineEmits<{
  "update:schedule": [schedule: number[]];
}>();

const { t } = useI18n();
const track = ref<HTMLDivElement | null>(null);
const dragging = ref<ShiftHandle | null>(null);
let stopDrag: (() => void) | null = null;

const TICKS = [0, 6, 12, 18, 24] as const;

/** 窗口由「非零班位」推导；0 是关班，不能当成已设置。 */
const shiftWindow = computed(() => deriveShiftWindow(props.schedule));
const startHour = computed(() => shiftWindow.value.start);
const endHour = computed(() => shiftWindow.value.end);

const shiftSlots = computed(() =>
  normalizeSchedule(props.schedule).map((value, index) => ({
    index,
    hour: index * SLOT_HOURS,
    active: value > 0,
    value,
  })),
);

/** 指针位置换算成小时。grabOffset 抵消按下点与柄中心的偏差，避免起拖跳位。 */
function hourFromEvent(event: PointerEvent, grabOffset: number): number {
  const rect = track.value?.getBoundingClientRect();
  if (!rect || rect.width === 0) return 0;
  const x = event.clientX - rect.left - grabOffset;
  const ratio = Math.min(Math.max(x / rect.width, 0), 1);
  return ratio * DAY_HOURS;
}

function onHandleDown(handle: ShiftHandle, event: PointerEvent): void {
  if (props.disabled) return;
  // 别让 Vue Flow 把这次按下当成节点拖动，也别选中文字
  event.preventDefault();
  event.stopPropagation();

  const rect = track.value?.getBoundingClientRect();
  const anchor = handle === "start" ? startHour.value : endHour.value;
  const grabOffset =
    rect && rect.width > 0
      ? event.clientX - rect.left - (anchor / DAY_HOURS) * rect.width
      : 0;

  const move = (moveEvent: PointerEvent) => {
    const next = moveShiftHandle(
      shiftWindow.value,
      handle,
      hourFromEvent(moveEvent, grabOffset),
    );
    emit("update:schedule", applyShiftWindow(props.schedule, next));
  };
  const finish = () => {
    dragging.value = null;
    stop();
  };
  const stop = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", finish);
    window.removeEventListener("pointercancel", finish);
    if (stopDrag === stop) stopDrag = null;
  };

  dragging.value = handle;
  stopDrag?.();
  stopDrag = stop;
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", finish);
  window.addEventListener("pointercancel", finish);
}

onBeforeUnmount(() => {
  stopDrag?.();
  stopDrag = null;
});

function label(hour: number): string {
  return `${hour}:00`;
}
</script>

<template>
  <div class="shift-slider nodrag nowheel" :class="{ disabled }">
    <div ref="track" class="shift-track">
      <div
        v-for="slot in shiftSlots"
        :key="slot.index"
        class="shift-slot"
        :class="{ active: slot.active }"
        :title="`${label(slot.hour)} - ${label(slot.hour + SLOT_HOURS)}：${
          slot.active
            ? t('studio.assetWorkbench.scheduleSlotActive', { count: slot.value })
            : t('studio.assetWorkbench.scheduleSlotOff')
        }`"
      >
        <span v-if="slot.active">{{ slot.value }}</span>
      </div>
      <div
        class="shift-window"
        :style="{ left: `${(startHour / DAY_HOURS) * 100}%`, width: `${((endHour - startHour) / DAY_HOURS) * 100}%` }"
      />
      <button
        type="button"
        class="shift-handle"
        :class="{ dragging: dragging === 'start' }"
        :style="{ left: `${(startHour / DAY_HOURS) * 100}%` }"
        :disabled="disabled"
        :aria-label="`${t('studio.assetWorkbench.shiftStart')} ${label(startHour)}`"
        @pointerdown="onHandleDown('start', $event)"
      >
        <span class="shift-badge start">{{ t("studio.assetWorkbench.shiftStart") }} {{ label(startHour) }}</span>
      </button>
      <button
        type="button"
        class="shift-handle"
        :class="{ dragging: dragging === 'end' }"
        :style="{ left: `${(endHour / DAY_HOURS) * 100}%` }"
        :disabled="disabled"
        :aria-label="`${t('studio.assetWorkbench.shiftEnd')} ${label(endHour)}`"
        @pointerdown="onHandleDown('end', $event)"
      >
        <span class="shift-badge end">{{ t("studio.assetWorkbench.shiftEnd") }} {{ label(endHour) }}</span>
      </button>
    </div>
    <div class="shift-ticks" aria-hidden="true">
      <span
        v-for="hour in TICKS"
        :key="hour"
        class="shift-tick"
        :style="{ left: `${(hour / DAY_HOURS) * 100}%` }"
      >
        {{ label(hour) }}
      </span>
    </div>
  </div>
</template>

<style scoped>
.shift-slider {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 4px;
  /* 留出柄宽的一半：0:00 / 24:00 的柄不会压在容器边缘外 */
  padding: 0 8px;
  width: 100%;
}
.shift-slider.disabled {
  opacity: 0.55;
  pointer-events: none;
}
.shift-track,
.shift-handle {
  touch-action: none;
  user-select: none;
}
.shift-track {
  position: relative;
  display: flex;
  height: 30px;
  border-radius: 6px;
  overflow: visible;
  background: var(--surface-sunken, rgba(255, 255, 255, 0.06));
  outline: 1px solid rgba(255, 255, 255, 0.12);
}
.shift-slot {
  flex: 1 1 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  color: var(--text-muted, rgba(255, 255, 255, 0.45));
  border-right: 1px dashed rgba(255, 255, 255, 0.1);
  border-radius: 4px;
}
.shift-slot.active {
  background: rgba(64, 158, 255, 0.28);
  color: var(--text-strong, rgba(255, 255, 255, 0.92));
}
.shift-window {
  position: absolute;
  top: 0;
  bottom: 0;
  background: rgba(64, 158, 255, 0.1);
  border-left: 2px solid rgba(64, 158, 255, 0.8);
  border-right: 2px solid rgba(64, 158, 255, 0.8);
  pointer-events: none;
}
.shift-handle {
  position: absolute;
  top: 50%;
  width: 14px;
  height: 22px;
  transform: translate(-50%, -50%);
  border-radius: 4px;
  border: 1px solid rgba(255, 255, 255, 0.35);
  background: linear-gradient(180deg, #4aa3ff, #2b7fd4);
  cursor: ew-resize;
  padding: 0;
  z-index: 1;
}
.shift-handle.dragging,
.shift-handle:focus-visible {
  outline: 2px solid rgba(120, 190, 255, 0.9);
}
.shift-badge {
  position: absolute;
  bottom: 26px;
  left: 50%;
  transform: translateX(-50%);
  white-space: nowrap;
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(20, 32, 48, 0.92);
  border: 1px solid rgba(120, 190, 255, 0.5);
  color: var(--text-strong, rgba(255, 255, 255, 0.92));
}
.shift-badge.end {
  bottom: auto;
  top: 26px;
}
.shift-ticks {
  position: relative;
  height: 13px;
  font-size: 10px;
  color: var(--text-muted, rgba(255, 255, 255, 0.4));
}
.shift-tick {
  position: absolute;
  top: 0;
  transform: translateX(-50%);
  white-space: nowrap;
}
</style>
