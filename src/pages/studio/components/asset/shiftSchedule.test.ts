import { describe, expect, it } from "vitest";
import {
  BASE_SCHEDULE,
  DAY_HOURS,
  SLOT_COUNT,
  SLOT_HOURS,
  applyShiftWindow,
  deriveShiftWindow,
  moveShiftHandle,
  normalizeSchedule,
  snapHour,
} from "./shiftSchedule";

describe("normalizeSchedule", () => {
  it("pads and trims to 8 slots, treating 0/negative/junk as off", () => {
    expect(normalizeSchedule([6, undefined, -1, Number.NaN, 7])).toEqual([
      6, 0, 0, 0, 7, 0, 0, 0,
    ]);
    expect(normalizeSchedule([1, 2, 3, 4, 5, 6, 7, 8, 9])).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
    expect(normalizeSchedule("junk")).toEqual(new Array(SLOT_COUNT).fill(0));
  });
});

describe("deriveShiftWindow", () => {
  it("reads the envelope of non-zero slots", () => {
    // 全时段在班：默认排班 6/7 交替 8 班位
    expect(deriveShiftWindow([...BASE_SCHEDULE])).toEqual({ start: 0, end: DAY_HOURS });
    expect(deriveShiftWindow([6, 6, 7, 7, 6, 6, 0, 0])).toEqual({ start: 0, end: 18 });
    expect(deriveShiftWindow([0, 0, 6, 6, 6, 0, 0, 0])).toEqual({ start: 6, end: 15 });
  });

  it("does not treat 0 (closed slot) as a set slot", () => {
    // 这是旧实现的回归点：0 被当成「已设置」，窗口永远 0–24，柄拖不动
    expect(deriveShiftWindow([0, 0, 0, 0, 0, 0, 0, 6])).toEqual({ start: 21, end: 24 });
    expect(deriveShiftWindow([0, 0, 0, 0, 0, 0, 0, 0])).toEqual({ start: 6, end: 18 });
  });

  it("spans holes inside the window instead of collapsing", () => {
    expect(deriveShiftWindow([6, 0, 6, 0, 0, 6, 0, 0])).toEqual({ start: 0, end: 18 });
  });
});

describe("snapHour", () => {
  it("snaps to 3-hour slot boundaries and clamps to the day", () => {
    expect(snapHour(4)).toBe(3);
    expect(snapHour(5)).toBe(6);
    expect(snapHour(-3)).toBe(0);
    expect(snapHour(DAY_HOURS + 6)).toBe(DAY_HOURS);
    expect(snapHour(Number.NaN)).toBe(0);
  });
});

describe("moveShiftHandle", () => {
  it("moves the start handle but keeps at least one slot", () => {
    expect(moveShiftHandle({ start: 6, end: 18 }, "start", 0)).toEqual({ start: 0, end: 18 });
    expect(moveShiftHandle({ start: 6, end: 18 }, "start", 12)).toEqual({ start: 12, end: 18 });
    // 越过交班时刻时被顶在 end - 1 班
    expect(moveShiftHandle({ start: 6, end: 18 }, "start", 24)).toEqual({ start: 15, end: 18 });
  });

  it("moves the end handle but keeps at least one slot", () => {
    expect(moveShiftHandle({ start: 6, end: 18 }, "end", 24)).toEqual({ start: 6, end: 24 });
    expect(moveShiftHandle({ start: 6, end: 18 }, "end", 0)).toEqual({ start: 6, end: 9 });
  });
});

describe("applyShiftWindow", () => {
  it("closes slots outside the window and keeps in-window headcounts", () => {
    expect(applyShiftWindow([...BASE_SCHEDULE], { start: 6, end: 18 })).toEqual([
      0, 0, 7, 7, 6, 6, 0, 0,
    ]);
    expect(applyShiftWindow([1, 2, 3, 4, 5, 6, 7, 8], { start: 6, end: 18 })).toEqual([
      0, 0, 3, 4, 5, 6, 0, 0,
    ]);
  });

  it("fills newly included slots with the base pattern", () => {
    expect(applyShiftWindow([0, 0, 0, 0, 0, 0, 0, 0], { start: 0, end: DAY_HOURS })).toEqual([
      ...BASE_SCHEDULE,
    ]);
  });

  it("round-trips: the derived window equals the dragged window", () => {
    const windows = [
      { start: 0, end: 24 },
      { start: 3, end: 21 },
      { start: 9, end: 12 },
      { start: 18, end: 24 },
    ];
    for (const window of windows) {
      const next = applyShiftWindow([...BASE_SCHEDULE], window);
      expect(next).toHaveLength(SLOT_COUNT);
      expect(deriveShiftWindow(next)).toEqual(window);
      // 相邻两次拖动不会继续改写窗口内的人数
      expect(applyShiftWindow(next, window)).toEqual(next);
    }
  });

  it("is idempotent when re-applied to its own output", () => {
    const once = applyShiftWindow([3, 4, 5, 6, 7, 8, 9, 10], { start: 6, end: 15 });
    expect(once).toEqual([0, 0, 5, 6, 7, 0, 0, 0]);
    expect(applyShiftWindow(once, { start: 6, end: 15 })).toEqual(once);
  });

  it("clamps windows that are not slot-aligned via the slot overlap rule", () => {
    // 5:00 起算时，3–6 点的班位仍有重叠 → 该班位在班
    expect(applyShiftWindow([...BASE_SCHEDULE], { start: 5, end: 9 })).toEqual([
      0, 6, 7, 0, 0, 0, 0, 0,
    ]);
  });
});

describe("constants", () => {
  it("describe a 24h day of 3h slots", () => {
    expect(SLOT_COUNT * SLOT_HOURS).toBe(DAY_HOURS);
    expect(BASE_SCHEDULE).toHaveLength(SLOT_COUNT);
  });
});
