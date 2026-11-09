/**
 * Path 折线分组（纯函数）：`0x0CAA6841` pathPairs 把点列切成多条路径。
 *
 * 引擎语义（CreatePathsFromProps / path_reconstruct_probe）：
 * - pairs 为偶数个 Int32，每对 `(start,end)` 含端点；
 * - start/end 是 `0x0CAA680D` 点数组下标（即 PathPointUnit.index）；
 * - 对数 = 路径数；奇数个为数据错误，回退单折线。
 *
 * 编辑器此前按 pointIndex 全局排序后画一条线，多路径 lot 会串线。
 */

export type PathPointLike = {
  index: number;
  point: [number, number, number] | null;
  pointIndex?: number | null;
};

/** 把 pathPairs 解析为若干闭区间；非法/奇数尾丢弃。 */
export function parsePathPairRanges(
  pathPairs: readonly number[],
): Array<{ start: number; end: number }> {
  const ranges: Array<{ start: number; end: number }> = [];
  if (pathPairs.length < 2) return ranges;
  const even = pathPairs.length - (pathPairs.length % 2);
  for (let i = 0; i < even; i += 2) {
    const start = pathPairs[i];
    const end = pathPairs[i + 1];
    if (
      typeof start !== "number" ||
      typeof end !== "number" ||
      !Number.isFinite(start) ||
      !Number.isFinite(end) ||
      end < start
    ) {
      continue;
    }
    ranges.push({ start, end });
  }
  return ranges;
}

/**
 * 按 pathPairs 分组 pathPoint；仅同组内点构成折线。
 * pairs 缺失或无法产出 ≥2 点的组时，回退为按 pointIndex/index 排序的单折线。
 */
export function groupPathPointPolylines<T extends PathPointLike>(
  points: readonly T[],
  pathPairs: readonly number[] = [],
): T[][] {
  const withPoint = points.filter((point) => point.point != null);
  if (withPoint.length === 0) return [];

  const ranges = parsePathPairRanges(pathPairs);
  if (ranges.length > 0) {
    const byIndex = new Map<number, T>();
    for (const point of withPoint) {
      byIndex.set(point.index, point);
    }
    const groups: T[][] = [];
    for (const { start, end } of ranges) {
      const group: T[] = [];
      for (let index = start; index <= end; index += 1) {
        const point = byIndex.get(index);
        if (point) group.push(point);
      }
      if (group.length >= 2) groups.push(group);
    }
    if (groups.length > 0) return groups;
  }

  return [
    [...withPoint].sort(
      (a, b) => (a.pointIndex ?? a.index) - (b.pointIndex ?? b.index),
    ),
  ];
}
