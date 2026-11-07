/**
 * 排班拖动条的纯数据层（2026-11-07）。
 *
 * 排班 = 8 个班位 × 3 小时，`0` 表示关班（与 unit 排班列
 * 0x0D00853A + i 的取值一一对应）。所有函数显式入参/返回值，
 * 不持有模块级状态，便于单测和回放。
 */

export const SLOT_HOURS = 3;
export const DAY_HOURS = 24;
export const SLOT_COUNT = DAY_HOURS / SLOT_HOURS;
/** 窗口新纳入班位的基准人数（与默认排班一致：6/7 交替）。 */
export const BASE_SCHEDULE: readonly number[] = [6, 6, 7, 7, 6, 6, 7, 6];

export type ShiftHandle = "start" | "end";

export interface ShiftWindow {
  /** 上班时刻（小时，0–24，落在班位边界）。 */
  start: number;
  /** 交班时刻（小时，0–24，落在班位边界）。 */
  end: number;
}

const isOnShift = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value) && value > 0;

/** 归一化为固定 8 班位；缺位、负数、非数值一律按关班（0）。 */
export function normalizeSchedule(schedule: unknown): number[] {
  const source: readonly unknown[] = Array.isArray(schedule) ? schedule : [];
  return Array.from({ length: SLOT_COUNT }, (_, index) => {
    const value = source[index];
    return isOnShift(value) ? value : 0;
  });
}

/**
 * 由非零班位推导窗口（包围首末在班班位）。
 * 全关班时回退为 6:00–18:00——只用于显示，不回写数据。
 */
export function deriveShiftWindow(schedule: unknown): ShiftWindow {
  const slots = normalizeSchedule(schedule);
  const first = slots.findIndex(isOnShift);
  if (first < 0) return { start: 6, end: 18 };
  let last = first;
  for (let index = first + 1; index < slots.length; index += 1) {
    if (isOnShift(slots[index])) last = index;
  }
  return { start: first * SLOT_HOURS, end: (last + 1) * SLOT_HOURS };
}

/** 把任意小时吸附到班位边界（0/3/…/24）。 */
export function snapHour(hour: number): number {
  if (!Number.isFinite(hour)) return 0;
  const clamped = Math.min(Math.max(hour, 0), DAY_HOURS);
  return Math.round(clamped / SLOT_HOURS) * SLOT_HOURS;
}

/** 拖动单个柄后的窗口：至少保留 1 个班位，且不越出 0–24。 */
export function moveShiftHandle(
  current: ShiftWindow,
  handle: ShiftHandle,
  hour: number,
): ShiftWindow {
  const target = snapHour(hour);
  if (handle === "start") {
    const start = Math.max(0, Math.min(target, current.end - SLOT_HOURS));
    return { start, end: current.end };
  }
  const end = Math.min(DAY_HOURS, Math.max(target, current.start + SLOT_HOURS));
  return { start: current.start, end };
}

/**
 * 按窗口重排 8 个班位：窗口外关班；窗口内保留原人数，
 * 新纳入的班位回落到基准值（不覆盖用户已有的班位规模）。
 */
export function applyShiftWindow(schedule: unknown, window: ShiftWindow): number[] {
  const slots = normalizeSchedule(schedule);
  return slots.map((value, index) => {
    const slotStart = index * SLOT_HOURS;
    const slotEnd = slotStart + SLOT_HOURS;
    const inside = slotStart < window.end && slotEnd > window.start;
    if (!inside) return 0;
    return isOnShift(value) ? value : BASE_SCHEDULE[index];
  });
}
