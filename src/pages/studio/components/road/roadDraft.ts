export const stripWidths = {
  forward: 3.2,
  backward: 3.2,
  walk: 3,
  margin: 1,
  median: 2,
  trees: 2,
  rail: 6,
} as const;

export type StripKind = keyof typeof stripWidths;
export const stripKinds = Object.keys(stripWidths) as StripKind[];
export interface RoadStrip {
  id: string;
  kind: StripKind;
  width: number;
}

export const stripDragType = "application/x-openscp-road-strip";
export const moveStripDragType = "application/x-openscp-move-road-strip";

export function isStripKind(value: unknown): value is StripKind {
  return typeof value === "string" && Object.hasOwn(stripWidths, value);
}

export function isValidStripWidth(width: unknown): width is number {
  return typeof width === "number" && Number.isFinite(width) && width > 0;
}

export function isRoadDraft(value: unknown): value is RoadStrip[] {
  if (!Array.isArray(value)) return false;
  const ids = new Set<string>();
  return value.every((strip) => {
    if (
      !strip ||
      typeof strip.id !== "string" ||
      !strip.id ||
      ids.has(strip.id) ||
      !isStripKind(strip.kind) ||
      !isValidStripWidth(strip.width)
    )
      return false;
    ids.add(strip.id);
    return true;
  });
}

export function insertStrip(
  strips: readonly RoadStrip[],
  index: number,
  kind: StripKind,
  width: number = stripWidths[kind],
): RoadStrip[] {
  const next = strips.slice();
  next.splice(index, 0, {
    id: crypto.randomUUID(),
    kind,
    width,
  });
  return next;
}

export function moveStrip(
  strips: readonly RoadStrip[],
  id: string,
  gap: number,
): RoadStrip[] {
  const from = strips.findIndex((strip) => strip.id === id);
  if (from < 0 || gap === from || gap === from + 1) return strips.slice();
  const next = strips.slice();
  next.splice(from, 1);
  next.splice(gap > from ? gap - 1 : gap, 0, strips[from]);
  return next;
}
