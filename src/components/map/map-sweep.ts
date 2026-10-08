import type { RoadSweep } from "@/lib/region-map";

/** GetSweepInfo / GenerateExtrusionModel: zero interval spans the entire path;
 * negative length anchors a short end cap at the end of each interval. */
export function sweepSections(
  s: RoadSweep,
  pathLength: number,
  modelLength: number,
) {
  let interval = s.interval || pathLength - s.start - s.end;
  if (interval <= 0 || modelLength <= 0) return [];
  let start = s.start,
    end = s.end;
  let length = s.length || interval;
  let step = s.step || (s.distort && s.repeatUv ? 8 : modelLength);
  if (s.roundIntervals) {
    const rounded =
      pathLength / Math.max(1, Math.round(pathLength / interval)) - 1 / 65536;
    const ratio = rounded / interval;
    interval = rounded;
    start *= ratio;
    end *= ratio;
    length *= ratio;
    step *= ratio;
  }
  if (interval <= 0 || step <= 0) return [];
  if (length < 0) start += interval - Math.abs(length);
  const result: { start: number; length: number }[] = [];
  for (let d = start; d < pathLength - end; d += interval) {
    if (s.instance) {
      result.push({ start: d, length: 0 });
      continue;
    }
    const stop = Math.min(pathLength - end, d + Math.abs(length));
    if (s.roundIntervals && d + Math.abs(length) > pathLength - end + 1 / 65536)
      break;
    for (let a = d; a < stop - 1e-7; a += step)
      result.push({ start: a, length: Math.min(step, stop - a) });
  }
  return result;
}
